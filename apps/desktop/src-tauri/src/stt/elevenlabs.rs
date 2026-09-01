use crate::error::{AppError, AppResult};
use crate::models::TranscriptSegment;
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use std::path::Path;
use uuid::Uuid;

pub const ELEVENLABS_STT_URL: &str = "https://api.elevenlabs.io/v1/speech-to-text";

#[derive(Debug, Clone)]
pub struct ElevenLabsConfig {
    pub api_key: String,
    pub model_id: String,
    pub language_code: Option<String>,
    pub num_speakers: Option<u32>,
    pub diarize: bool,
    /// Domain glossary biasing transcription (persisted app-wide, see `Storage::get_glossary`).
    /// Sent as the `keyterms` multipart field; omitted entirely when empty.
    pub keyterms: Vec<String>,
}

impl Default for ElevenLabsConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            // scribe_v1 was removed from the ElevenLabs API on 2026-07-09.
            model_id: "scribe_v2".to_string(),
            language_code: Some("ko".to_string()),
            num_speakers: None,
            diarize: true,
            keyterms: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ElevenLabsResponse {
    #[serde(default)]
    words: Vec<ElevenLabsWord>,
}

#[derive(Debug, Deserialize)]
struct ElevenLabsWord {
    #[serde(default)]
    text: String,
    #[serde(rename = "type", default)]
    kind: String,
    start: Option<f64>,
    end: Option<f64>,
    #[serde(default)]
    speaker_id: Option<String>,
}

struct PendingSegment {
    speaker_id: Option<String>,
    start_ms: i64,
    end_ms: i64,
    text: String,
}

fn milliseconds(seconds: Option<f64>) -> i64 {
    (seconds.unwrap_or_default() * 1000.0) as i64
}

/// Bare Hangul jamo (ㄱ, ㅏ …) are letters to Unicode but never valid keyterms on their own.
fn is_hangul_jamo(c: char) -> bool {
    matches!(c, '\u{1100}'..='\u{11FF}' | '\u{3131}'..='\u{318E}' | '\u{A960}'..='\u{A97F}' | '\u{D7B0}'..='\u{D7FF}')
}

pub(crate) fn sanitize_keyterms(terms: &[String]) -> Vec<String> {
    terms
        .iter()
        .map(|term| term.trim())
        .filter(|term| {
            !term.is_empty()
                && term.chars().count() <= 50
                && term.chars().all(|character| !character.is_control())
                && term
                    .chars()
                    .any(|character| {
                        (character.is_alphabetic() && !is_hangul_jamo(character))
                            || character.is_numeric()
                    })
        })
        .map(str::to_owned)
        .fold(Vec::new(), |mut terms, term| {
            if !terms.contains(&term) {
                terms.push(term);
            }
            terms
        })
}

pub fn is_invalid_keyword_error(status: u16, body: &str) -> bool {
    status == 400
        && (body.contains(r#""status":"invalid_keyword""#)
            || body.contains(r#""param":"keywords""#))
}

fn speaker_label(speaker_id: Option<&str>) -> String {
    speaker_id
        .and_then(|id| id.strip_prefix("speaker_"))
        .and_then(|index| index.parse::<u32>().ok())
        .map(|index| format!("화자 {}", index + 1))
        .unwrap_or_else(|| "화자 미확인".to_string())
}

fn finish_segment(recording_id: Uuid, pending: PendingSegment) -> TranscriptSegment {
    TranscriptSegment {
        id: Uuid::new_v4(),
        recording_id,
        start_ms: pending.start_ms,
        end_ms: pending.end_ms,
        speaker_label: speaker_label(pending.speaker_id.as_deref()),
        text: pending.text.trim_start().to_string(),
        is_final: true,
    }
}

pub fn segments_from_response(recording_id: Uuid, body: &str) -> AppResult<Vec<TranscriptSegment>> {
    let response: ElevenLabsResponse = serde_json::from_str(body)
        .map_err(|error| AppError::Stt(format!("invalid ElevenLabs response: {error}")))?;

    let mut segments = Vec::new();
    let mut pending: Option<PendingSegment> = None;

    for word in response.words {
        if word.kind != "word" || word.text.trim().is_empty() {
            continue;
        }

        let speaker_changed = pending
            .as_ref()
            .is_some_and(|segment| segment.speaker_id != word.speaker_id);
        if speaker_changed {
            segments.push(finish_segment(recording_id, pending.take().unwrap()));
        }

        let word_start_ms = milliseconds(word.start);
        let word_end_ms = milliseconds(word.end);
        let segment = pending.get_or_insert_with(|| PendingSegment {
            speaker_id: word.speaker_id,
            start_ms: word_start_ms,
            end_ms: word_end_ms,
            text: String::new(),
        });
        segment.end_ms = word_end_ms;
        segment.text.push_str(&word.text);
    }

    if let Some(segment) = pending {
        segments.push(finish_segment(recording_id, segment));
    }

    Ok(segments)
}

/// The non-file multipart fields of a batch transcription request. Split out from
/// [`build_request`] so tests can assert the wire contract without a network call
/// (a built reqwest multipart body is an opaque stream).
pub fn text_fields(cfg: &ElevenLabsConfig) -> Vec<(&'static str, String)> {
    let mut fields = vec![
        ("model_id", cfg.model_id.clone()),
        ("diarize", cfg.diarize.to_string()),
        ("timestamps_granularity", "word".to_string()),
    ];
    if let Some(num_speakers) = cfg.num_speakers {
        fields.push(("num_speakers", num_speakers.to_string()));
    }
    if let Some(language_code) = &cfg.language_code {
        fields.push(("language_code", language_code.clone()));
    }
    let keyterms = sanitize_keyterms(&cfg.keyterms);
    if !keyterms.is_empty() {
        fields.push((
            "keyterms",
            serde_json::to_string(&keyterms).unwrap_or_else(|_| "[]".to_string()),
        ));
    }
    fields
}

pub fn build_request(
    client: &reqwest::Client,
    cfg: &ElevenLabsConfig,
    file_bytes: Vec<u8>,
    filename: &str,
) -> reqwest::RequestBuilder {
    let mut form = Form::new().part(
        "file",
        Part::bytes(file_bytes).file_name(filename.to_string()),
    );
    for (name, value) in text_fields(cfg) {
        form = form.text(name, value);
    }

    client
        .post(ELEVENLABS_STT_URL)
        .header("xi-api-key", &cfg.api_key)
        .multipart(form)
}

pub async fn transcribe_wav_file(
    cfg: &ElevenLabsConfig,
    recording_id: Uuid,
    wav_path: &Path,
) -> AppResult<Vec<TranscriptSegment>> {
    let file_bytes = tokio::fs::read(wav_path).await?;
    let filename = wav_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("audio.wav");
    let client = reqwest::Client::new();
    let mut response = build_request(&client, cfg, file_bytes.clone(), filename)
        .send()
        .await?;
    let mut status = response.status();
    let mut body = response.text().await?;

    if is_invalid_keyword_error(status.as_u16(), &body)
        && !sanitize_keyterms(&cfg.keyterms).is_empty()
    {
        eprintln!("ElevenLabs rejected keyterms; retrying transcription without them");
        let mut retry_cfg = cfg.clone();
        retry_cfg.keyterms.clear();
        response = build_request(&client, &retry_cfg, file_bytes, filename)
            .send()
            .await?;
        status = response.status();
        body = response.text().await?;
    }

    if !status.is_success() {
        return Err(AppError::Stt(format!(
            "ElevenLabs request failed with status {status}: {body}"
        )));
    }

    segments_from_response(recording_id, &body)
}
