//! Client for the self-hosted WhisperLive STT + online speaker-diarization server
//! (infra/stt-server/, deployed on 192.168.1.189 GPU1). Talks the WhisperLive WebSocket
//! protocol: send a JSON session-config handshake, stream raw f32 PCM frames, receive JSON
//! transcription messages with per-segment `start`/`end`/`text`/`speaker`/`completed`.

use crate::error::{AppError, AppResult};
use crate::models::TranscriptSegment;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::error::ProtocolError;
use tokio_tungstenite::tungstenite::{Error as WebSocketError, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
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

#[derive(Debug, Clone, Serialize)]
pub struct TranscriptionProgress {
    pub recording_id: Uuid,
    pub sent_ms: i64,
    pub total_ms: i64,
    pub phase: ProgressPhase,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgressPhase {
    Sending,
    Finalizing,
    Done,
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

struct ReceivedSegment {
    start_ms: i64,
    end_ms: i64,
    speaker_label: String,
    text: String,
}

type SttWebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WebSocketWrite = SplitSink<SttWebSocket, Message>;
type WebSocketRead = SplitStream<SttWebSocket>;

fn spawn_reader(
    mut read: WebSocketRead,
    recording_offset_ms: i64,
    segment_sender: UnboundedSender<ReceivedSegment>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(msg) = read.next().await {
            let msg = match msg {
                Ok(message) => message,
                Err(_) => break,
            };
            match msg {
                Message::Text(text) => {
                    if text == "DISCONNECT" || text == "END_OF_AUDIO" {
                        break;
                    }
                    if let Ok(parsed) = serde_json::from_str::<ServerMessage>(&text) {
                        if let Some(segments) = parsed.segments {
                            for segment in segments {
                                if !segment.completed {
                                    continue;
                                }
                                let (Ok(start), Ok(end)) =
                                    (segment.start.parse::<f64>(), segment.end.parse::<f64>())
                                else {
                                    continue;
                                };
                                let _ = segment_sender.send(ReceivedSegment {
                                    start_ms: recording_offset_ms + (start * 1000.0) as i64,
                                    end_ms: recording_offset_ms + (end * 1000.0) as i64,
                                    speaker_label: segment
                                        .speaker
                                        .unwrap_or_else(|| "화자 미확인".to_string()),
                                    text: segment.text.trim().to_string(),
                                });
                            }
                        }
                        if parsed.message.as_deref() == Some("DISCONNECT") {
                            break;
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    })
}

async fn open_session(
    cfg: &SttConfig,
    handshake_json: &str,
    recording_offset_ms: i64,
    segment_sender: UnboundedSender<ReceivedSegment>,
) -> Result<(WebSocketWrite, JoinHandle<()>), String> {
    let (ws_stream, _) = tokio_tungstenite::connect_async(&cfg.ws_url)
        .await
        .map_err(|error| format!("connect to {}: {error}", cfg.ws_url))?;
    let (mut write, read) = ws_stream.split();

    // Start polling before sending the handshake so keepalive Pings are always answered while
    // a recording is streamed at real-time speed.
    let reader = spawn_reader(read, recording_offset_ms, segment_sender);
    if let Err(error) = write.send(Message::Text(handshake_json.to_string())).await {
        reader.abort();
        let _ = reader.await;
        return Err(format!("send handshake: {error}"));
    }

    Ok((write, reader))
}

fn is_retryable_connection_error(error: &WebSocketError) -> bool {
    matches!(
        error,
        WebSocketError::Io(_)
            | WebSocketError::Tls(_)
            | WebSocketError::ConnectionClosed
            | WebSocketError::AlreadyClosed
            | WebSocketError::Protocol(
                ProtocolError::SendAfterClosing
                    | ProtocolError::ReceivedAfterClosing
                    | ProtocolError::ResetWithoutClosingHandshake
            )
    )
}

async fn reconnect_after_send_error(
    cfg: &SttConfig,
    handshake_json: &str,
    recording_offset_ms: i64,
    segment_sender: UnboundedSender<ReceivedSegment>,
    session: (WebSocketWrite, JoinHandle<()>),
    send_context: &str,
    send_error: WebSocketError,
) -> AppResult<(WebSocketWrite, JoinHandle<()>)> {
    let (write, reader) = session;
    drop(write);
    reader.abort();
    let _ = reader.await;

    open_session(cfg, handshake_json, recording_offset_ms, segment_sender)
        .await
        .map_err(|reconnect_error| {
            AppError::WebSocket(format!(
                "{send_context} failed ({send_error}); reconnect attempt failed: {reconnect_error}"
            ))
        })
}

fn pcm_frame(samples: &[f32]) -> Message {
    let mut bytes = Vec::with_capacity(samples.len() * 4);
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    Message::Binary(bytes)
}

fn send_progress(
    progress_sender: Option<&UnboundedSender<TranscriptionProgress>>,
    recording_id: Uuid,
    sent_ms: i64,
    total_ms: i64,
    phase: ProgressPhase,
) {
    if let Some(sender) = progress_sender {
        let _ = sender.send(TranscriptionProgress {
            recording_id,
            sent_ms,
            total_ms,
            phase,
        });
    }
}

/// Transcribes a 16-bit mono PCM WAV file end-to-end against the WhisperLive server: opens a
/// WebSocket, sends the diarization-enabled session config, streams the decoded PCM as f32
/// frames, and collects every `completed` segment into `TranscriptSegment`s.
pub async fn transcribe_wav_file(
    cfg: &SttConfig,
    recording_id: Uuid,
    wav_path: &std::path::Path,
    progress_sender: Option<UnboundedSender<TranscriptionProgress>>,
) -> AppResult<Vec<TranscriptSegment>> {
    let mut wav_reader = hound::WavReader::open(wav_path)
        .map_err(|error| AppError::Audio(format!("failed to open wav {wav_path:?}: {error}")))?;
    let spec = wav_reader.spec();
    let samples_f32: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => wav_reader
            .samples::<i16>()
            .map(|sample| sample.map(|value| value as f32 / i16::MAX as f32))
            .collect::<Result<_, _>>()
            .map_err(|error| AppError::Audio(format!("wav decode error: {error}")))?,
        hound::SampleFormat::Float => wav_reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|error| AppError::Audio(format!("wav decode error: {error}")))?,
    };
    let total_ms = samples_f32.len() as i64 * 1000 / i64::from(spec.sample_rate);

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
        .map_err(|error| AppError::WebSocket(format!("serialize handshake: {error}")))?;

    let (segment_sender, mut segment_receiver) = tokio::sync::mpsc::unbounded_channel();
    let (mut write, mut reader) = open_session(cfg, &handshake_json, 0, segment_sender.clone())
        .await
        .map_err(AppError::WebSocket)?;

    // WhisperLive runs VAD + ASR on an internal timer against audio received so far, so these
    // chunks MUST remain paced close to real time. Blasting the file races the processing thread
    // and produces incomplete or empty transcription results.
    const CHUNK: usize = 4096;
    let chunk_duration = std::time::Duration::from_secs_f64(CHUNK as f64 / spec.sample_rate as f64);
    let audio_chunk_count = samples_f32.len().div_ceil(CHUNK);
    let mut audio_chunk_index = 0;
    let mut retried = false;

    while audio_chunk_index < audio_chunk_count {
        let chunk_start = audio_chunk_index * CHUNK;
        let chunk_end = (chunk_start + CHUNK).min(samples_f32.len());
        match write
            .send(pcm_frame(&samples_f32[chunk_start..chunk_end]))
            .await
        {
            Ok(()) => {
                audio_chunk_index += 1;
                let sent_samples = (audio_chunk_index * CHUNK).min(samples_f32.len());
                let sent_ms = sent_samples as i64 * 1000 / i64::from(spec.sample_rate);
                send_progress(
                    progress_sender.as_ref(),
                    recording_id,
                    sent_ms,
                    total_ms,
                    ProgressPhase::Sending,
                );
                tokio::time::sleep(chunk_duration).await;
            }
            Err(error) if is_retryable_connection_error(&error) && !retried => {
                let recording_offset_ms = chunk_start as i64 * 1000 / i64::from(spec.sample_rate);
                (write, reader) = reconnect_after_send_error(
                    cfg,
                    &handshake_json,
                    recording_offset_ms,
                    segment_sender.clone(),
                    (write, reader),
                    "send audio chunk",
                    error,
                )
                .await?;
                retried = true;
            }
            Err(error) if is_retryable_connection_error(&error) => {
                return Err(AppError::WebSocket(format!(
                    "send audio chunk failed after one reconnect: {error}"
                )));
            }
            Err(error) => {
                return Err(AppError::WebSocket(format!("send audio chunk: {error}")));
            }
        }
    }

    // A few seconds of trailing silence gives VAD room to flush the final real-audio segment.
    const TRAILING_SILENCE_SECS: f64 = 3.0;
    let silence_chunk = vec![0.0_f32; CHUNK];
    let trailing_chunk_count =
        ((TRAILING_SILENCE_SECS * spec.sample_rate as f64) / CHUNK as f64).ceil() as usize;
    let mut trailing_chunk_index = 0;
    while trailing_chunk_index < trailing_chunk_count {
        match write.send(pcm_frame(&silence_chunk)).await {
            Ok(()) => {
                trailing_chunk_index += 1;
                tokio::time::sleep(chunk_duration).await;
            }
            Err(error) if is_retryable_connection_error(&error) && !retried => {
                (write, reader) = reconnect_after_send_error(
                    cfg,
                    &handshake_json,
                    total_ms,
                    segment_sender.clone(),
                    (write, reader),
                    "send trailing silence",
                    error,
                )
                .await?;
                retried = true;
            }
            Err(error) if is_retryable_connection_error(&error) => {
                return Err(AppError::WebSocket(format!(
                    "send trailing silence failed after one reconnect: {error}"
                )));
            }
            Err(error) => {
                return Err(AppError::WebSocket(format!(
                    "send trailing silence: {error}"
                )));
            }
        }
    }

    // WhisperLive expects END_OF_AUDIO as a binary frame; a text frame is decoded as PCM and
    // crashes the server's receive path.
    write
        .send(Message::Binary(b"END_OF_AUDIO".to_vec()))
        .await
        .map_err(|error| AppError::WebSocket(format!("send eos: {error}")))?;
    send_progress(
        progress_sender.as_ref(),
        recording_id,
        total_ms,
        total_ms,
        ProgressPhase::Finalizing,
    );
    let _ = write.close().await;

    // The server flushes final segments and closes after END_OF_AUDIO. Bound the wait in case it
    // never closes, then merge the growing segment snapshots from both connection attempts. The
    // latest version of each absolute (start, end) pair wins.
    match tokio::time::timeout(std::time::Duration::from_secs(30), &mut reader).await {
        Ok(join_result) => join_result
            .map_err(|error| AppError::WebSocket(format!("reader task panicked: {error}")))?,
        Err(_) => {
            reader.abort();
            let _ = reader.await;
            return Err(AppError::WebSocket(
                "timed out waiting for final transcription segments".to_string(),
            ));
        }
    }

    drop(segment_sender);
    let mut completed = std::collections::BTreeMap::new();
    while let Some(segment) = segment_receiver.recv().await {
        completed.insert((segment.start_ms, segment.end_ms), segment);
    }
    let result = completed
        .into_values()
        .map(|segment| TranscriptSegment {
            id: Uuid::new_v4(),
            recording_id,
            start_ms: segment.start_ms,
            end_ms: segment.end_ms,
            speaker_label: segment.speaker_label,
            text: segment.text,
            is_final: true,
        })
        .collect();

    send_progress(
        progress_sender.as_ref(),
        recording_id,
        total_ms,
        total_ms,
        ProgressPhase::Done,
    );
    Ok(result)
}
