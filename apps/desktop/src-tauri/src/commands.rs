//! Tauri command surface exposed to the webview via IPC. Every command returns
//! `Result<T, AppError>` (AppError serializes to a display string, see error.rs).

use crate::audio::CaptureSession;
use crate::error::{AppError, AppResult};
use crate::minutes;
use crate::models::{MinutesDraft, MinutesItem, Recording, RecordingStatus, TranscriptSegment};
use crate::storage::Storage;
use crate::stt::{self, SttConfig};
use std::path::PathBuf;
use std::process::Command as ShellCommand;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

pub struct AppState {
    pub storage: Storage,
    pub capture: Mutex<Option<(Uuid, CaptureSession)>>,
}

fn recordings_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::InvalidState(format!("no app data dir: {e}")))?
        .join("recordings");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[tauri::command]
pub async fn start_recording(app: AppHandle, state: State<'_, AppState>, title: String) -> AppResult<Recording> {
    let id = Uuid::new_v4();
    let wav_path = recordings_dir(&app)?.join(format!("{id}.wav"));
    let session = CaptureSession::start(wav_path.clone())?;

    let rec = Recording {
        id,
        title,
        source_path: wav_path.to_string_lossy().to_string(),
        duration_ms: None,
        status: RecordingStatus::Recording,
        created_at: chrono::Utc::now(),
    };
    state.storage.insert_recording(&rec).await?;
    *state.capture.lock().unwrap() = Some((id, session));
    Ok(rec)
}

#[tauri::command]
pub async fn stop_recording(state: State<'_, AppState>) -> AppResult<Recording> {
    let (id, session) = state
        .capture
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| AppError::InvalidState("no recording in progress".to_string()))?;
    let (_, duration_ms) = session.stop()?;
    state.storage.set_duration(id, duration_ms).await?;
    state.storage.update_status(id, RecordingStatus::Recorded).await?;
    state
        .storage
        .get_recording(id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("recording {id} vanished after stop")))
}

#[tauri::command]
pub async fn list_recordings(state: State<'_, AppState>) -> AppResult<Vec<Recording>> {
    state.storage.list_recordings().await
}

#[tauri::command]
pub async fn get_recording_detail(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<(Recording, Vec<TranscriptSegment>)> {
    let uuid = Uuid::parse_str(&id).map_err(|e| AppError::InvalidState(format!("bad id: {e}")))?;
    let rec = state
        .storage
        .get_recording(uuid)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("recording {id} not found")))?;
    let segments = state.storage.list_segments(uuid).await?;
    Ok((rec, segments))
}

/// Runs STT + online speaker diarization against the self-hosted WhisperLive server
/// (infra/stt-server/) for an already-recorded WAV, persists the resulting segments, and
/// flips the recording status. Used both for live captures and for ingested test files.
#[tauri::command]
pub async fn transcribe_recording(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> AppResult<Vec<TranscriptSegment>> {
    let uuid = Uuid::parse_str(&id).map_err(|e| AppError::InvalidState(format!("bad id: {e}")))?;
    let rec = state
        .storage
        .get_recording(uuid)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("recording {id} not found")))?;

    state.storage.update_status(uuid, RecordingStatus::Transcribing).await?;
    let cfg = SttConfig::default();
    let wav_path = PathBuf::from(&rec.source_path);
    let (progress_sender, mut progress_receiver) =
        tokio::sync::mpsc::unbounded_channel::<stt::TranscriptionProgress>();
    tokio::spawn(async move {
        while let Some(progress) = progress_receiver.recv().await {
            let _ = app.emit("transcription-progress", &progress);
        }
    });
    let result =
        stt::transcribe_wav_file(&cfg, uuid, &wav_path, Some(progress_sender)).await;
    match result {
        Ok(segments) => {
            state.storage.insert_segments(&segments).await?;
            state.storage.update_status(uuid, RecordingStatus::Transcribed).await?;
            Ok(segments)
        }
        Err(e) => {
            state.storage.update_status(uuid, RecordingStatus::Failed).await?;
            Err(e)
        }
    }
}

/// Ingests an existing audio file (e.g. from the local test-data corpus) that is NOT part of
/// this repo — the caller passes an absolute path outside the workspace. Converts to 16kHz
/// mono PCM WAV via macOS's built-in `afconvert` (no bundled ffmpeg dependency) and registers
/// it as a new recording, ready for `transcribe_recording`.
#[tauri::command]
pub async fn ingest_audio_file(app: AppHandle, state: State<'_, AppState>, source_path: String, title: String) -> AppResult<Recording> {
    let src = PathBuf::from(&source_path);
    if !src.exists() {
        return Err(AppError::NotFound(format!("source file not found: {source_path}")));
    }
    let id = Uuid::new_v4();
    let wav_path = recordings_dir(&app)?.join(format!("{id}.wav"));

    let status = ShellCommand::new("afconvert")
        .args([
            "-f", "WAVE",
            "-d", "LEI16@16000",
            "-c", "1",
        ])
        .arg(&src)
        .arg(&wav_path)
        .status()
        .map_err(|e| AppError::Audio(format!("failed to spawn afconvert: {e}")))?;
    if !status.success() {
        return Err(AppError::Audio(format!("afconvert failed converting {source_path}")));
    }

    let duration_ms = {
        let reader = hound::WavReader::open(&wav_path)
            .map_err(|e| AppError::Audio(format!("failed to read converted wav: {e}")))?;
        let spec = reader.spec();
        let frames = reader.len() as f64;
        (frames / spec.sample_rate as f64 * 1000.0) as i64
    };

    let rec = Recording {
        id,
        title,
        source_path: wav_path.to_string_lossy().to_string(),
        duration_ms: Some(duration_ms),
        status: RecordingStatus::Recorded,
        created_at: chrono::Utc::now(),
    };
    state.storage.insert_recording(&rec).await?;
    Ok(rec)
}

#[tauri::command]
pub async fn generate_minutes(
    state: State<'_, AppState>,
    recording_id: String,
) -> AppResult<MinutesDraft> {
    let recording_id = Uuid::parse_str(&recording_id)
        .map_err(|error| AppError::InvalidState(format!("bad recording id: {error}")))?;
    let segments = state.storage.list_segments(recording_id).await?;
    let draft = minutes::generate_minutes(recording_id, &segments).await?;
    state.storage.save_minutes(&draft).await?;
    Ok(draft)
}

/// Returns the previously saved minutes draft for a recording, if `generate_minutes` has ever
/// been run for it. Lets the frontend restore an existing draft instead of losing it and forcing
/// a fresh (costly) LLM generation every time the recording detail is reopened.
#[tauri::command]
pub async fn get_minutes(
    state: State<'_, AppState>,
    recording_id: String,
) -> AppResult<Option<MinutesDraft>> {
    let recording_id = Uuid::parse_str(&recording_id)
        .map_err(|error| AppError::InvalidState(format!("bad recording id: {error}")))?;
    state.storage.get_minutes(recording_id).await
}

#[tauri::command]
pub async fn edit_minutes_item(
    state: State<'_, AppState>,
    recording_id: String,
    item_id: String,
    instruction: String,
) -> AppResult<MinutesItem> {
    let recording_id = Uuid::parse_str(&recording_id)
        .map_err(|error| AppError::InvalidState(format!("bad recording id: {error}")))?;
    let item_id = Uuid::parse_str(&item_id)
        .map_err(|error| AppError::InvalidState(format!("bad minutes item id: {error}")))?;
    let mut draft = state
        .storage
        .get_minutes(recording_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!("minutes for recording {recording_id} not found"))
        })?;
    let original = draft
        .decisions
        .iter()
        .chain(draft.action_items.iter())
        .find(|item| item.id == item_id)
        .cloned()
        .ok_or_else(|| AppError::NotFound(format!("minutes item {item_id} not found")))?;

    let segments = state.storage.list_segments(recording_id).await?;
    let evidence_segments = segments
        .into_iter()
        .filter(|segment| original.evidence_segment_ids.contains(&segment.id))
        .collect::<Vec<_>>();
    let replacement_text =
        minutes::edit_minutes_item_text(&original, &instruction, &evidence_segments).await?;

    let edited = draft
        .decisions
        .iter_mut()
        .chain(draft.action_items.iter_mut())
        .find(|item| item.id == item_id)
        .ok_or_else(|| AppError::NotFound(format!("minutes item {item_id} not found")))?;
    edited.text = replacement_text;
    let edited = edited.clone();
    draft.updated_at = chrono::Utc::now();
    state.storage.save_minutes(&draft).await?;
    Ok(edited)
}
