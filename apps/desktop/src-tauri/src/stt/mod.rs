//! Client for the self-hosted WhisperLive STT + online speaker-diarization server
//! (infra/stt-server/, deployed on 192.168.1.189 GPU1). Talks the WhisperLive WebSocket
//! protocol: send a JSON session-config handshake, stream raw f32 PCM frames, receive JSON
//! transcription messages with per-segment `start`/`end`/`text`/`speaker`/`completed`.

use crate::error::{AppError, AppResult};
use crate::models::TranscriptSegment;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SttConfig {
    pub ws_url: String,
    pub language: String,
    pub model: String,
    pub max_speakers: u32,
    pub diarization_threshold: f32,
}

impl Default for SttConfig {
    fn default() -> Self {
        Self {
            ws_url: "ws://192.168.1.189:9090".to_string(),
            language: "ko".to_string(),
            model: "large-v3-turbo".to_string(),
            max_speakers: 4,
            diarization_threshold: 0.55,
        }
    }
}

#[derive(Serialize)]
struct SessionConfig<'a> {
    uid: String,
    language: &'a str,
    task: &'a str,
    model: &'a str,
    use_vad: bool,
    enable_diarization: bool,
    max_speakers: u32,
    diarization_threshold: f32,
}

#[derive(Deserialize, Debug)]
struct ServerSegment {
    start: f64,
    end: f64,
    text: String,
    #[serde(default)]
    speaker: Option<String>,
    #[serde(default)]
    completed: bool,
}

#[derive(Deserialize, Debug)]
struct ServerMessage {
    #[serde(default)]
    segments: Option<Vec<ServerSegment>>,
    #[serde(default)]
    message: Option<String>,
}

/// Transcribes a 16-bit mono PCM WAV file end-to-end against the WhisperLive server: opens a
/// WebSocket, sends the diarization-enabled session config, streams the decoded PCM as f32
/// frames, and collects every `completed` segment into `TranscriptSegment`s.
pub async fn transcribe_wav_file(
    cfg: &SttConfig,
    recording_id: Uuid,
    wav_path: &std::path::Path,
) -> AppResult<Vec<TranscriptSegment>> {
    let mut reader = hound::WavReader::open(wav_path)
        .map_err(|e| AppError::Audio(format!("failed to open wav {wav_path:?}: {e}")))?;
    let spec = reader.spec();
    let samples_f32: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => reader
            .samples::<i16>()
            .map(|s| s.map(|v| v as f32 / i16::MAX as f32))
            .collect::<Result<_, _>>()
            .map_err(|e| AppError::Audio(format!("wav decode error: {e}")))?,
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|e| AppError::Audio(format!("wav decode error: {e}")))?,
    };

    let (ws_stream, _) = tokio_tungstenite::connect_async(&cfg.ws_url)
        .await
        .map_err(|e| AppError::WebSocket(format!("connect to {}: {e}", cfg.ws_url)))?;
    let (mut write, mut read) = ws_stream.split();

    let handshake = SessionConfig {
        uid: recording_id.to_string(),
        language: &cfg.language,
        task: "transcribe",
        model: &cfg.model,
        use_vad: true,
        enable_diarization: true,
        max_speakers: cfg.max_speakers,
        diarization_threshold: cfg.diarization_threshold,
    };
    let handshake_json = serde_json::to_string(&handshake)
        .map_err(|e| AppError::WebSocket(format!("serialize handshake: {e}")))?;
    write
        .send(Message::Text(handshake_json))
        .await
        .map_err(|e| AppError::WebSocket(format!("send handshake: {e}")))?;

    // Stream PCM in ~4096-sample chunks (~256ms at 16kHz) as raw little-endian f32 bytes, the
    // format WhisperLive's client reference implementation uses over the websocket.
    const CHUNK: usize = 4096;
    for chunk in samples_f32.chunks(CHUNK) {
        let mut bytes = Vec::with_capacity(chunk.len() * 4);
        for s in chunk {
            bytes.extend_from_slice(&s.to_le_bytes());
        }
        write
            .send(Message::Binary(bytes))
            .await
            .map_err(|e| AppError::WebSocket(format!("send audio chunk: {e}")))?;
    }
    // Signal end-of-stream per WhisperLive protocol.
    write
        .send(Message::Text("END_OF_AUDIO".to_string()))
        .await
        .map_err(|e| AppError::WebSocket(format!("send eos: {e}")))?;

    let mut completed: Vec<ServerSegment> = Vec::new();
    while let Some(msg) = read.next().await {
        let msg = msg.map_err(|e| AppError::WebSocket(format!("recv: {e}")))?;
        match msg {
            Message::Text(text) => {
                if text == "DISCONNECT" || text == "END_OF_AUDIO" {
                    break;
                }
                if let Ok(parsed) = serde_json::from_str::<ServerMessage>(&text) {
                    if let Some(segs) = parsed.segments {
                        for seg in segs {
                            if seg.completed {
                                completed.push(seg);
                            }
                        }
                    }
                    if let Some(m) = parsed.message {
                        if m == "DISCONNECT" {
                            break;
                        }
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
    let _ = write.close().await;

    Ok(completed
        .into_iter()
        .map(|s| TranscriptSegment {
            id: Uuid::new_v4(),
            recording_id,
            start_ms: (s.start * 1000.0) as i64,
            end_ms: (s.end * 1000.0) as i64,
            speaker_label: s.speaker.unwrap_or_else(|| "화자 미확인".to_string()),
            text: s.text.trim().to_string(),
            is_final: true,
        })
        .collect())
}
