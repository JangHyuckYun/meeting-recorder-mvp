pub mod audio;
mod commands;
pub mod error;
pub mod models;
pub mod storage;
pub mod stt;

use commands::AppState;
use std::sync::Mutex;
use storage::Storage;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
                });
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_recording,
            commands::stop_recording,
            commands::list_recordings,
            commands::get_recording_detail,
            commands::transcribe_recording,
            commands::ingest_audio_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
