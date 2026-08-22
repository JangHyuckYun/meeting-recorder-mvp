//! Application-wide error type. Every fallible Tauri command returns `AppResult<T>` so the
//! frontend gets a stable, serializable error message instead of a panic across the IPC boundary.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("audio device error: {0}")]
    Audio(String),
    #[error("storage error: {0}")]
    Storage(#[from] sqlx::Error),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("websocket error: {0}")]
    WebSocket(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid state: {0}")]
    InvalidState(String),
}

pub type AppResult<T> = Result<T, AppError>;

// Tauri commands require the error type to implement `serde::Serialize` to cross the IPC
// boundary to the webview; we serialize to the display message only (no internal detail leak).
impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
