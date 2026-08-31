pub mod asr;
pub mod audio;
mod commands;
pub mod error;
pub mod minutes;
pub mod models;
pub mod storage;
pub mod stt;

use commands::AppState;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::Mutex;
use storage::Storage;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("app data dir must be resolvable");
            std::fs::create_dir_all(&data_dir).expect("failed to create app data dir");
            let db_path = data_dir.join("meeting-recorder.sqlite3");

            tauri::async_runtime::block_on(async move {
                let storage = Storage::connect(&db_path)
                    .await
                    .expect("failed to open sqlite storage");
                handle.manage(AppState {
                    storage,
                    capture: Mutex::new(None),
                    transcription_cancel: Arc::new(AtomicBool::new(false)),
                });
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_recording,
            commands::stop_recording,
            commands::list_recordings,
            commands::get_recording_detail,
            commands::delete_recording,
            commands::transcribe_recording,
            commands::ingest_audio_file,
            commands::generate_minutes,
            commands::get_minutes,
            commands::edit_minutes_item,
            commands::get_app_settings,
            commands::set_app_settings,
            commands::set_elevenlabs_api_key,
            commands::list_templates,
            commands::create_template,
            commands::update_template,
            commands::delete_template,
            commands::list_folders,
            commands::create_folder,
            commands::delete_folder,
            commands::assign_recording_folder,
            commands::get_speaker_names,
            commands::set_speaker_name,
            commands::get_glossary,
            commands::set_glossary,
            commands::get_oauth_status,
            commands::start_oauth_login,
            commands::list_providers,
            commands::add_provider,
            commands::update_provider,
            commands::delete_provider,
            commands::get_model_assignments,
            commands::set_model_assignment,
            commands::cancel_transcription,
            commands::update_recording_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
