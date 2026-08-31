//! Tauri command surface exposed to the webview via IPC. Every command returns
//! `Result<T, AppError>` (AppError serializes to a display string, see error.rs).

use crate::audio::capture::CaptureSession;
use crate::error::{AppError, AppResult};
use crate::minutes;
use crate::models::{
    AppSettings, LlmProvider, MinutesDraft, MinutesItem, Recording, RecordingStatus, SttEngine,
    TranscriptSegment,
};
use crate::storage::Storage;
use crate::stt::{self, SttConfig};
use std::path::PathBuf;
use std::process::Command as ShellCommand;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::AsyncBufReadExt;
use uuid::Uuid;

pub struct AppState {
    pub storage: Storage,
    pub capture: Mutex<Option<(Uuid, CaptureSession)>>,
    pub transcription_cancel: Arc<AtomicBool>,
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
pub async fn start_recording(
    app: AppHandle,
    state: State<'_, AppState>,
    title: String,
) -> AppResult<Recording> {
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
        folder_id: None,
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
    state
        .storage
        .update_status(id, RecordingStatus::Recorded)
        .await?;
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

/// Updates the status of a recording (used for retry/reset).
#[tauri::command]
pub async fn update_recording_status(
    state: State<'_, AppState>,
    id: String,
    status: String,
) -> AppResult<()> {
    let uuid = Uuid::parse_str(&id).map_err(|e| AppError::InvalidState(format!("bad id: {e}")))?;
    let rec_status = RecordingStatus::from_db_str(&status);
    state.storage.update_status(uuid, rec_status).await
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

/// Deletes a recording, its transcript/minutes rows, and its archived WAV file. A missing
/// audio file is not an error (an ingest may have been cleaned up already); a missing
/// recording row is `NotFound`.
#[tauri::command]
pub async fn delete_recording(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let uuid = Uuid::parse_str(&id).map_err(|e| AppError::InvalidState(format!("bad id: {e}")))?;
    let rec = state
        .storage
        .get_recording(uuid)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("recording {id} not found")))?;
    state.storage.delete_recording(uuid).await?;
    let _ = std::fs::remove_file(&rec.source_path);
    Ok(())
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

    state
        .storage
        .update_status(uuid, RecordingStatus::Transcribing)
        .await?;
    let settings = state.storage.get_settings().await?;
    let wav_path = PathBuf::from(&rec.source_path);

    let result = match settings.stt_engine {
        SttEngine::Elevenlabs => {
            let api_key = state
                .storage
                .elevenlabs_api_key()
                .await?
                .filter(|key| !key.trim().is_empty());
            if let Some(api_key) = api_key {
                let cfg = stt::elevenlabs::ElevenLabsConfig {
                    api_key,
                    language_code: Some("ko".to_string()),
                    num_speakers: None,
                    keyterms: state.storage.get_glossary().await?,
                    ..Default::default()
                };
                let total_ms = rec.duration_ms.unwrap_or_default();
                let _ = app.emit(
                    "transcription-progress",
                    stt::TranscriptionProgress {
                        recording_id: uuid,
                        sent_ms: total_ms,
                        total_ms,
                        phase: stt::ProgressPhase::Finalizing,
                    },
                );
                let result = stt::elevenlabs::transcribe_wav_file(&cfg, uuid, &wav_path).await;
                if result.is_ok() {
                    let _ = app.emit(
                        "transcription-progress",
                        stt::TranscriptionProgress {
                            recording_id: uuid,
                            sent_ms: total_ms,
                            total_ms,
                            phase: stt::ProgressPhase::Done,
                        },
                    );
                }
                result
            } else {
                Err(AppError::InvalidState(
                    "ElevenLabs API 키가 설정되지 않았습니다".to_string(),
                ))
            }
        }
        SttEngine::SelfHosted => {
            let ws_url = settings
                .stt_server_url
                .unwrap_or_else(|| "ws://192.168.1.189:9090".to_string());
            let cfg = SttConfig {
                ws_url,
                ..SttConfig::default()
            };
            let (progress_sender, mut progress_receiver) =
                tokio::sync::mpsc::unbounded_channel::<stt::TranscriptionProgress>();
            tokio::spawn(async move {
                while let Some(progress) = progress_receiver.recv().await {
                    let _ = app.emit("transcription-progress", &progress);
                }
            });

            // Reset the cancel flag before starting
            state.transcription_cancel.store(false, Ordering::SeqCst);
            let cancel_flag = state.transcription_cancel.clone();

            stt::transcribe_wav_file(
                &cfg,
                uuid,
                &wav_path,
                Some(progress_sender),
                Some(cancel_flag),
            )
            .await
        }
    };
    match result {
        Ok(segments) => {
            state.storage.insert_segments(&segments).await?;
            state
                .storage
                .update_status(uuid, RecordingStatus::Transcribed)
                .await?;
            Ok(segments)
        }
        Err(e) => {
            // Check if cancelled — set status back to recorded so retry works
            if e.to_string().contains("cancelled") {
                state
                    .storage
                    .update_status(uuid, RecordingStatus::Recorded)
                    .await?;
            } else {
                state
                    .storage
                    .update_status(uuid, RecordingStatus::Failed)
                    .await?;
            }
            Err(e)
        }
    }
}

/// Cancels the currently running transcription.
#[tauri::command]
pub async fn cancel_transcription(state: State<'_, AppState>) -> AppResult<()> {
    state.transcription_cancel.store(true, Ordering::SeqCst);
    Ok(())
}

/// Ingests an existing audio file (e.g. from the local test-data corpus) that is NOT part of
/// this repo — the caller passes an absolute path outside the workspace. Converts to 16kHz
/// mono PCM WAV via macOS's built-in `afconvert` (no bundled ffmpeg dependency) and registers
/// it as a new recording, ready for `transcribe_recording`.
#[tauri::command]
pub async fn ingest_audio_file(
    app: AppHandle,
    state: State<'_, AppState>,
    source_path: String,
    title: String,
) -> AppResult<Recording> {
    let src = PathBuf::from(&source_path);
    if !src.exists() {
        return Err(AppError::NotFound(format!(
            "source file not found: {source_path}"
        )));
    }
    let id = Uuid::new_v4();
    let wav_path = recordings_dir(&app)?.join(format!("{id}.wav"));

    let status = ShellCommand::new("afconvert")
        .args(["-f", "WAVE", "-d", "LEI16@16000", "-c", "1"])
        .arg(&src)
        .arg(&wav_path)
        .status()
        .map_err(|e| AppError::Audio(format!("failed to spawn afconvert: {e}")))?;
    if !status.success() {
        return Err(AppError::Audio(format!(
            "afconvert failed converting {source_path}"
        )));
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
        folder_id: None,
    };
    state.storage.insert_recording(&rec).await?;
    Ok(rec)
}

#[tauri::command]
pub async fn generate_minutes(
    state: State<'_, AppState>,
    recording_id: String,
    template_id: Option<String>,
) -> AppResult<MinutesDraft> {
    use crate::minutes::ResolvedProvider;

    let recording_id = Uuid::parse_str(&recording_id)
        .map_err(|error| AppError::InvalidState(format!("bad recording id: {error}")))?;
    let segments = state.storage.list_segments(recording_id).await?;
    let template = load_template_content(&state.storage, template_id.as_deref()).await?;
    let template = template.as_deref();

    // Try model_assignments table first; fall back to stored_llm_provider for backward compat.
    if let Some((provider_id, model_name)) = state
        .storage
        .get_assigned_provider_model("minutes_generation")
        .await?
    {
        if let Some(provider) = state.storage.get_provider(provider_id).await? {
            // Built-in OAuth providers use CLI credentials, not direct API keys.
            // Route them through the existing OAuth path.
            if provider.is_builtin {
                let llm = match provider.provider_type.as_str() {
                    "openai" => LlmProvider::CodexOauth,
                    "anthropic" => LlmProvider::ClaudeOauth,
                    _ => stored_llm_provider(&state.storage).await?,
                };
                let draft =
                    minutes::generate_minutes(llm, recording_id, &segments, template).await?;
                state.storage.save_minutes(&draft).await?;
                return Ok(draft);
            }

            let resolved = ResolvedProvider {
                provider_type: crate::models::ProviderType::from_db_str(&provider.provider_type)
                    .unwrap_or(crate::models::ProviderType::OpenaiCompatible),
                base_url: provider.base_url.clone(),
                api_key: provider.api_key_masked.clone(),
                model: model_name,
            };
            let draft = minutes::generate_minutes_with_resolved(
                resolved,
                recording_id,
                &segments,
                template,
            )
            .await?;
            state.storage.save_minutes(&draft).await?;
            return Ok(draft);
        }
    }

    // Fallback: stored llm_provider setting
    let llm_provider = stored_llm_provider(&state.storage).await?;
    let draft = minutes::generate_minutes(llm_provider, recording_id, &segments, template).await?;
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

/// Returns the user-managed app settings, applying defaults for keys never saved. Backs the
/// settings UI's initial state.
#[tauri::command]
pub async fn get_app_settings(state: State<'_, AppState>) -> AppResult<AppSettings> {
    state.storage.get_settings().await
}

/// Reads the persisted LLM provider, falling back to the LiteLLM default for keys never saved
/// or holding a value written by an older build.
async fn stored_llm_provider(storage: &Storage) -> AppResult<LlmProvider> {
    let stored = storage.get_setting("llm_provider").await?;
    Ok(stored
        .as_deref()
        .and_then(LlmProvider::from_db_str)
        .unwrap_or_default())
}

/// Persists settings changed from the settings UI. Applies to the NEXT minutes generation or
/// edit; an in-flight LLM call finishes on its original provider.
#[tauri::command]
pub async fn set_app_settings(state: State<'_, AppState>, settings: AppSettings) -> AppResult<()> {
    state
        .storage
        .set_setting("llm_provider", settings.llm_provider.as_str())
        .await?;
    if let Some(url) = &settings.stt_server_url {
        state.storage.set_setting("stt_server_url", url).await?;
    }
    state
        .storage
        .set_setting("stt_engine", settings.stt_engine.as_str())
        .await?;
    Ok(())
}

/// Persists the ElevenLabs secret separately from the settings roundtrip.
#[tauri::command]
pub async fn set_elevenlabs_api_key(state: State<'_, AppState>, api_key: String) -> AppResult<()> {
    state
        .storage
        .set_setting("elevenlabs_api_key", &api_key)
        .await
}

// ------------------------------------------------------------------
// Minutes templates
// ------------------------------------------------------------------

/// Resolves an optional template id to its content. An absent/blank id means "no template";
/// an id that does not exist is an error so the user is not silently given ungoverned minutes.
async fn load_template_content(
    storage: &Storage,
    template_id: Option<&str>,
) -> AppResult<Option<String>> {
    let Some(id) = template_id.map(str::trim).filter(|id| !id.is_empty()) else {
        return Ok(None);
    };
    let uuid =
        Uuid::parse_str(id).map_err(|e| AppError::InvalidState(format!("bad template id: {e}")))?;
    let template = storage
        .get_template(uuid)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("template {id} not found")))?;
    Ok(Some(template.content))
}

#[tauri::command]
pub async fn list_templates(state: State<'_, AppState>) -> AppResult<Vec<crate::models::Template>> {
    state.storage.list_templates().await
}

#[tauri::command]
pub async fn create_template(
    state: State<'_, AppState>,
    name: String,
    content: String,
) -> AppResult<crate::models::Template> {
    state.storage.create_template(&name, &content).await
}

#[tauri::command]
pub async fn update_template(
    state: State<'_, AppState>,
    id: String,
    name: String,
    content: String,
) -> AppResult<()> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|e| AppError::InvalidState(format!("bad template id: {e}")))?;
    state.storage.update_template(uuid, &name, &content).await
}

#[tauri::command]
pub async fn delete_template(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|e| AppError::InvalidState(format!("bad template id: {e}")))?;
    state.storage.delete_template(uuid).await
}

// ------------------------------------------------------------------
// Folders
// ------------------------------------------------------------------

#[tauri::command]
pub async fn list_folders(state: State<'_, AppState>) -> AppResult<Vec<crate::models::Folder>> {
    state.storage.list_folders().await
}

#[tauri::command]
pub async fn create_folder(
    state: State<'_, AppState>,
    name: String,
) -> AppResult<crate::models::Folder> {
    state.storage.create_folder(&name).await
}

/// Deletes a folder. Recordings filed under it are unfiled, never deleted.
#[tauri::command]
pub async fn delete_folder(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let uuid =
        Uuid::parse_str(&id).map_err(|e| AppError::InvalidState(format!("bad folder id: {e}")))?;
    state.storage.delete_folder(uuid).await
}

/// Files a recording under a folder; `folderId: null` unfiles it.
#[tauri::command]
pub async fn assign_recording_folder(
    state: State<'_, AppState>,
    recording_id: String,
    folder_id: Option<String>,
) -> AppResult<()> {
    let recording_uuid = Uuid::parse_str(&recording_id)
        .map_err(|e| AppError::InvalidState(format!("bad recording id: {e}")))?;
    let folder_uuid = match folder_id.as_deref().filter(|id| !id.is_empty()) {
        Some(id) => Some(
            Uuid::parse_str(id)
                .map_err(|e| AppError::InvalidState(format!("bad folder id: {e}")))?,
        ),
        None => None,
    };
    state
        .storage
        .assign_recording_folder(recording_uuid, folder_uuid)
        .await
}

/// Answers a question grounded in one recording's transcript and minutes, through the same
/// LLM provider the minutes pipeline uses (only the prompt differs). Citations are best-effort:
/// an answer the model failed to cite comes back with empty `sources`, never an error.
#[tauri::command]
pub async fn ask_note(
    state: State<'_, AppState>,
    recording_id: String,
    question: String,
) -> AppResult<crate::models::AskAnswer> {
    let uuid = Uuid::parse_str(&recording_id)
        .map_err(|error| AppError::InvalidState(format!("bad recording id: {error}")))?;
    let segments = state.storage.list_segments(uuid).await?;
    let minutes = state.storage.get_minutes(uuid).await?;
    let minutes = minutes.as_ref();

    if let Some((provider_id, model_name)) = state
        .storage
        .get_assigned_provider_model("minutes_generation")
        .await?
    {
        if let Some(provider) = state.storage.get_provider(provider_id).await? {
            if provider.is_builtin {
                let llm = match provider.provider_type.as_str() {
                    "openai" => LlmProvider::CodexOauth,
                    "anthropic" => LlmProvider::ClaudeOauth,
                    _ => stored_llm_provider(&state.storage).await?,
                };
                return minutes::ask_note(llm, &question, &segments, minutes).await;
            }
            let resolved = minutes::ResolvedProvider {
                provider_type: crate::models::ProviderType::from_db_str(&provider.provider_type)
                    .unwrap_or(crate::models::ProviderType::OpenaiCompatible),
                base_url: provider.base_url.clone(),
                api_key: provider.api_key_masked.clone(),
                model: model_name,
            };
            return minutes::ask_note_with_resolved(resolved, &question, &segments, minutes).await;
        }
    }

    let llm_provider = stored_llm_provider(&state.storage).await?;
    minutes::ask_note(llm_provider, &question, &segments, minutes).await
}

/// Writes the recording's transcript to the user's Downloads directory in one of
/// "srt" | "vtt" | "md" | "txt" and returns the absolute path written.
#[tauri::command]
pub async fn export_transcript(
    app: AppHandle,
    state: State<'_, AppState>,
    recording_id: String,
    format: String,
) -> AppResult<String> {
    let uuid = Uuid::parse_str(&recording_id)
        .map_err(|e| AppError::InvalidState(format!("bad recording id: {e}")))?;
    let format = crate::export::ExportFormat::from_str(&format)?;
    let rec = state
        .storage
        .get_recording(uuid)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("recording {recording_id} not found")))?;
    let segments = state.storage.list_segments(uuid).await?;
    if segments.is_empty() {
        return Err(AppError::InvalidState(format!(
            "recording {recording_id} has no transcript to export"
        )));
    }
    let names = state.storage.get_speaker_names(uuid).await?;
    let contents = crate::export::render(&rec.title, &segments, &names, format);

    let dir = app
        .path()
        .download_dir()
        .map_err(|e| AppError::InvalidState(format!("no downloads dir: {e}")))?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(crate::export::safe_filename(
        &rec.title,
        &uuid.to_string()[..8],
        format,
    ));
    std::fs::write(&path, contents)?;
    Ok(path.to_string_lossy().to_string())
}

/// Per-recording speaker display names, keyed by diarization label ("화자 1" → "김민지").
/// Empty when the user has never renamed anyone.
#[tauri::command]
pub async fn get_speaker_names(
    state: State<'_, AppState>,
    recording_id: String,
) -> AppResult<std::collections::BTreeMap<String, String>> {
    let uuid = Uuid::parse_str(&recording_id)
        .map_err(|e| AppError::InvalidState(format!("bad recording id: {e}")))?;
    state.storage.get_speaker_names(uuid).await
}

/// Renames one speaker for one recording. A blank `name` clears the override.
#[tauri::command]
pub async fn set_speaker_name(
    state: State<'_, AppState>,
    recording_id: String,
    speaker_key: String,
    name: String,
) -> AppResult<()> {
    let uuid = Uuid::parse_str(&recording_id)
        .map_err(|e| AppError::InvalidState(format!("bad recording id: {e}")))?;
    state
        .storage
        .set_speaker_name(uuid, &speaker_key, &name)
        .await
}

/// Returns the persisted STT glossary (keyterms). Empty when never set.
#[tauri::command]
pub async fn get_glossary(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    state.storage.get_glossary().await
}

/// Replaces the persisted STT glossary. Blank entries are dropped. Applies to the NEXT
/// transcription; an in-flight one keeps the glossary it started with.
#[tauri::command]
pub async fn set_glossary(state: State<'_, AppState>, terms: Vec<String>) -> AppResult<()> {
    state.storage.set_glossary(&terms).await
}

/// Inspects local OAuth credential stores for the settings UI.
#[tauri::command]
pub async fn get_oauth_status(
    provider: String,
) -> AppResult<crate::minutes::oauth_status::OAuthStatus> {
    crate::minutes::oauth_status::inspect(&provider)
}

/// Starts an interactive OAuth login flow for the selected provider. Spawns the
/// provider's CLI (`codex login` / `claude auth login`) in the background, streams
/// its stdout/stderr to the webview via `oauth-login-output` events, and emits
/// `oauth-login-url` when an https:// URL is detected so the UI can offer a
/// one-click "브라우저에서 열기". The CLI writes tokens to its standard credential
/// store on success; the UI should re-query `get_oauth_status` after receiving
/// `oauth-login-done`.
#[tauri::command]
pub async fn start_oauth_login(
    app: AppHandle,
    provider: String,
    method: Option<String>,
) -> AppResult<String> {
    let (binary, args): (&str, Vec<&str>) = match provider.as_str() {
        "codex_oauth" => {
            let use_device = method.as_deref() == Some("device");
            if use_device {
                ("codex", vec!["login", "--device-auth"])
            } else {
                ("codex", vec!["login"])
            }
        }
        "claude_oauth" => ("claude", vec!["auth", "login"]),
        other => {
            return Err(AppError::InvalidState(format!(
                "unknown oauth provider `{other}`; expected `codex_oauth` or `claude_oauth`"
            )))
        }
    };

    let app_clone = app.clone();
    let provider_clone = provider.clone();
    tokio::spawn(async move {
        let emit = |event: &str, payload: serde_json::Value| {
            let _ = app_clone.emit(event, payload);
        };

        let mut cmd = tokio::process::Command::new(binary);
        cmd.args(&args);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        if let Ok(home) = std::env::var("HOME") {
            let nvm_bin = format!("{home}/.nvm/versions/node/v24.14.0/bin");
            let current_path = std::env::var("PATH").unwrap_or_default();
            let augmented = format!("{nvm_bin}:/opt/homebrew/bin:/usr/local/bin:{current_path}");
            cmd.env("PATH", augmented);
        }

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(error) => {
                emit(
                    "oauth-login-output",
                    serde_json::json!({
                        "provider": provider_clone,
                        "line": format!("Failed to spawn `{binary}`: {error}. Is the CLI installed and on PATH?")
                    }),
                );
                emit(
                    "oauth-login-done",
                    serde_json::json!({"provider": provider_clone, "success": false}),
                );
                return;
            }
        };

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let provider_for_stdout = provider_clone.clone();
        let app_for_stdout = app_clone.clone();
        let stdout_task = tokio::spawn(async move {
            if let Some(stdout) = stdout {
                let mut reader = tokio::io::BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    if let Some(url_start) = line.find("https://") {
                        let url = line[url_start..]
                            .split_whitespace()
                            .next()
                            .unwrap_or("")
                            .trim_end_matches(|c| ['.', ',', ')', ']', '"', '\''].contains(&c));
                        if !url.is_empty() {
                            let _ = app_for_stdout.emit(
                                "oauth-login-url",
                                serde_json::json!({"provider": provider_for_stdout, "url": url}),
                            );
                        }
                    }
                    let _ = app_for_stdout.emit(
                        "oauth-login-output",
                        serde_json::json!({"provider": provider_for_stdout, "line": line}),
                    );
                }
            }
        });

        let provider_for_stderr = provider_clone.clone();
        let app_for_stderr = app_clone.clone();
        let stderr_task = tokio::spawn(async move {
            if let Some(stderr) = stderr {
                let mut reader = tokio::io::BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let _ = app_for_stderr.emit(
                        "oauth-login-output",
                        serde_json::json!({"provider": provider_for_stderr, "line": line}),
                    );
                }
            }
        });

        let status = child.wait().await;
        let _ = tokio::join!(stdout_task, stderr_task);
        let success = status.map(|s| s.success()).unwrap_or(false);
        emit(
            "oauth-login-done",
            serde_json::json!({"provider": provider_clone, "success": success}),
        );
        if success {
            emit(
                "oauth-login-output",
                serde_json::json!({"provider": provider_clone, "line": "Login process finished — credential status will refresh on next check."}),
            );
        }
    });

    Ok(format!("Spawning `{binary}` login flow…"))
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

    // Try model_assignments table first; fall back to stored_llm_provider.
    let replacement_text = if let Some((provider_id, model_name)) = state
        .storage
        .get_assigned_provider_model("minutes_edit")
        .await?
    {
        if let Some(provider) = state.storage.get_provider(provider_id).await? {
            if provider.is_builtin {
                let llm = match provider.provider_type.as_str() {
                    "openai" => LlmProvider::CodexOauth,
                    "anthropic" => LlmProvider::ClaudeOauth,
                    _ => stored_llm_provider(&state.storage).await?,
                };
                minutes::edit_minutes_item_text(llm, &original, &instruction, &evidence_segments)
                    .await?
            } else {
                use crate::minutes::ResolvedProvider;
                let resolved = ResolvedProvider {
                    provider_type: crate::models::ProviderType::from_db_str(
                        &provider.provider_type,
                    )
                    .unwrap_or(crate::models::ProviderType::OpenaiCompatible),
                    base_url: provider.base_url.clone(),
                    api_key: provider.api_key_masked.clone(),
                    model: model_name,
                };
                minutes::edit_minutes_item_text_with_resolved(
                    resolved,
                    &original,
                    &instruction,
                    &evidence_segments,
                )
                .await?
            }
        } else {
            let llm_provider = stored_llm_provider(&state.storage).await?;
            minutes::edit_minutes_item_text(
                llm_provider,
                &original,
                &instruction,
                &evidence_segments,
            )
            .await?
        }
    } else {
        let llm_provider = stored_llm_provider(&state.storage).await?;
        minutes::edit_minutes_item_text(llm_provider, &original, &instruction, &evidence_segments)
            .await?
    };

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

// ------------------------------------------------------------------
// Provider registry commands
// ------------------------------------------------------------------

/// Lists all registered providers, both built-in and user-added, with masked API keys.
#[tauri::command]
pub async fn list_providers(
    state: State<'_, AppState>,
) -> AppResult<Vec<crate::models::ProviderSummary>> {
    state.storage.list_providers().await
}

/// Adds a new user provider. Returns the generated provider id.
#[tauri::command]
pub async fn add_provider(
    state: State<'_, AppState>,
    input: crate::models::ProviderInput,
) -> AppResult<String> {
    let id = state.storage.add_provider(&input).await?;
    Ok(id.to_string())
}

/// Updates an existing (non-builtin) provider.
#[tauri::command]
pub async fn update_provider(
    state: State<'_, AppState>,
    input: crate::models::ProviderInput,
) -> AppResult<()> {
    state.storage.update_provider(&input).await
}

/// Deletes a non-builtin provider and its cascade model assignments.
#[tauri::command]
pub async fn delete_provider(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let id = Uuid::parse_str(&id)
        .map_err(|e| AppError::InvalidState(format!("bad provider id: {e}")))?;
    state.storage.delete_provider(id).await
}

/// Lists all model assignments (purpose → provider_id + model_name).
#[tauri::command]
pub async fn get_model_assignments(
    state: State<'_, AppState>,
) -> AppResult<Vec<crate::models::ModelAssignment>> {
    state.storage.list_model_assignments().await
}

/// Sets or updates a model assignment for one purpose.
#[tauri::command]
pub async fn set_model_assignment(
    state: State<'_, AppState>,
    input: crate::models::ModelAssignmentInput,
) -> AppResult<()> {
    state
        .storage
        .set_model_assignment(&input.purpose, &input.provider_id, &input.model_name)
        .await
}
