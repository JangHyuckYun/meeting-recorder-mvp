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
    send_last_n_segments: u32,
    no_speech_thresh: f32,
    clip_audio: bool,
    same_output_threshold: u32,
    enable_translation: bool,
    enable_diarization: bool,
    max_speakers: u32,
    diarization_threshold: f32,
    word_timestamps: bool,
    audio_format: &'a str,
}

#[derive(Deserialize, Debug)]
struct ServerSegment {
    // WhisperLive sends start/end as JSON strings (e.g. "3.450"), not numbers — deserializing
    // straight into f64 silently fails the whole ServerMessage parse (confirmed against the
    // live deployment: every segments message was dropped without a single logged error).
    start: String,
    end: String,
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
    let (mut write, read) = ws_stream.split();

    // The reader runs CONCURRENTLY with sending, not after it. tokio-tungstenite only answers
    // the server's keepalive Ping frames with a Pong while the stream is actively being polled;
    // for a multi-minute recording, sending everything first and only then starting to read
    // leaves Pings unanswered long enough that the server tears the connection down with a
    // "keepalive ping timeout" 1011 error before any segments are ever received (confirmed
    // against the live deployment with a 3-minute clip — a sequential send-then-read pipeline
    // died with a broken pipe around the 50s mark). Spawning the reader first, before the
    // handshake is even sent, keeps Pong responses flowing for the whole session.
    let reader_recording_id = recording_id;
    let reader = tokio::spawn(async move {
        let mut read = read;
        // WhisperLive resends the growing "last N segments" list on every message as the
        // sliding transcription window advances, so the same segment (same start/end) can
        // appear `completed: true` more than once — often with speaker/text refined between
        // resends. Key by (start, end) and overwrite so the LAST version (most refined) wins.
        let mut completed: std::collections::BTreeMap<(String, String), ServerSegment> =
            std::collections::BTreeMap::new();
        while let Some(msg) = read.next().await {
            let msg = match msg {
                Ok(m) => m,
                Err(_) => break,
            };
            match msg {
                Message::Text(text) => {
                    if text == "DISCONNECT" || text == "END_OF_AUDIO" {
                        break;
                    }
                    if let Ok(parsed) = serde_json::from_str::<ServerMessage>(&text) {
                        if let Some(segs) = parsed.segments {
                            for seg in segs {
                                if seg.completed {
                                    completed.insert((seg.start.clone(), seg.end.clone()), seg);
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

        let mut result: Vec<TranscriptSegment> = completed
            .into_values()
            .filter_map(|s| {
                let start = s.start.parse::<f64>().ok()?;
                let end = s.end.parse::<f64>().ok()?;
                Some(TranscriptSegment {
                    id: Uuid::new_v4(),
                    recording_id: reader_recording_id,
                    start_ms: (start * 1000.0) as i64,
                    end_ms: (end * 1000.0) as i64,
                    speaker_label: s.speaker.unwrap_or_else(|| "화자 미확인".to_string()),
                    text: s.text.trim().to_string(),
                    is_final: true,
                })
            })
            .collect();
        result.sort_by_key(|seg| seg.start_ms);
        result
    });

    let handshake = SessionConfig {
        uid: recording_id.to_string(),
        language: &cfg.language,
        task: "transcribe",
        model: &cfg.model,
        use_vad: true,
        send_last_n_segments: 50,
        no_speech_thresh: 0.7,
        clip_audio: false,
        same_output_threshold: 5,
        enable_translation: false,
        enable_diarization: true,
        max_speakers: cfg.max_speakers,
        diarization_threshold: cfg.diarization_threshold,
        word_timestamps: false,
        audio_format: "float32",
    };
    let handshake_json = serde_json::to_string(&handshake)
        .map_err(|e| AppError::WebSocket(format!("serialize handshake: {e}")))?;
    write
        .send(Message::Text(handshake_json))
        .await
        .map_err(|e| AppError::WebSocket(format!("send handshake: {e}")))?;

    // Stream PCM in ~4096-sample chunks (~256ms at 16kHz) as raw little-endian f32 bytes, the
    // format WhisperLive's client reference implementation uses over the websocket. WhisperLive
    // runs VAD + ASR on an internal timer against whatever audio has arrived so far, so chunks
    // MUST be paced close to real-time — blasting the whole file instantly races the server's
    // processing thread and yields zero segments (confirmed against the live deployment: only
    // the first ~1.3s of audio was ever buffered before the session tore down). A few seconds
    // of trailing silence after the real audio gives VAD room to flush the final segment before
    // END_OF_AUDIO, mirroring the reference client behavior that was verified to work.
    const CHUNK: usize = 4096;
    let chunk_duration = std::time::Duration::from_secs_f64(CHUNK as f64 / spec.sample_rate as f64);
    fn pcm_frame(samples: &[f32]) -> Message {
        let mut bytes = Vec::with_capacity(samples.len() * 4);
        for s in samples {
            bytes.extend_from_slice(&s.to_le_bytes());
        }
        Message::Binary(bytes)
    }
    for chunk in samples_f32.chunks(CHUNK) {
        write
            .send(pcm_frame(chunk))
            .await
            .map_err(|e| AppError::WebSocket(format!("send audio chunk: {e}")))?;
        tokio::time::sleep(chunk_duration).await;
    }

    const TRAILING_SILENCE_SECS: f64 = 3.0;
    let silence_chunk = vec![0.0f32; CHUNK];
    let trailing_chunks = ((TRAILING_SILENCE_SECS * spec.sample_rate as f64) / CHUNK as f64).ceil() as usize;
    for _ in 0..trailing_chunks {
        write
            .send(pcm_frame(&silence_chunk))
            .await
            .map_err(|e| AppError::WebSocket(format!("send trailing silence: {e}")))?;
        tokio::time::sleep(chunk_duration).await;
    }

    // Signal end-of-stream per WhisperLive protocol. This MUST be a binary frame — the server
    // decodes every incoming frame as raw audio bytes, and a text frame here crashes it with
    // "a bytes-like object is required, not 'str'" (confirmed against the live deployment).
    write
        .send(Message::Binary(b"END_OF_AUDIO".to_vec()))
        .await
        .map_err(|e| AppError::WebSocket(format!("send eos: {e}")))?;
    let _ = write.close().await;

    // The server flushes final segments and closes the socket after END_OF_AUDIO; the reader
    // task then returns. Bound the wait in case the server never sends a close frame.
    tokio::time::timeout(std::time::Duration::from_secs(30), reader)
        .await
        .map_err(|_| AppError::WebSocket("timed out waiting for final transcription segments".to_string()))?
        .map_err(|e| AppError::WebSocket(format!("reader task panicked: {e}")))
}
