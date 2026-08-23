//! ASR domain types re-exported from the crate's canonical model definitions.

use crate::error::{AppError, AppResult};
use crate::models::{CaptionEvent, CaptionStatus};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

/// Re-export of the canonical 48kHz sample clock.
pub use crate::models::AudioTime as DomainAudioTime;

/// Capture state descriptor surfaced to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureState {
    pub recording: bool,
    pub ring_buffer_seconds: f32,
    pub archive_path: Option<String>,
}

impl CaptureState {
    pub fn idle() -> Self {
        Self {
            recording: false,
            ring_buffer_seconds: 0.0,
            archive_path: None,
        }
    }
}

/// Minimal wrapper over the canonical [`crate::models::CaptionEvent`] for the LAN transport.
pub type DomainCaptionEvent = CaptionEvent;

/// Minimal wrapper over the canonical [`crate::models::CaptionStatus`].
pub type DomainCaptionStatus = CaptionStatus;

/// Connection result from the WS transport layer.
pub type ServerConnection = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Deserialize a caption event from the gateway's JSON over a connection.
pub fn parse_gateway_message(raw: &str) -> AppResult<Option<CaptionEvent>> {
    if raw.trim().is_empty() {
        return Ok(None);
    }
    serde_json::from_str(raw)
        .map(Some)
        .map_err(|e| AppError::Stt(format!("failed to parse gateway message: {e}")))
}

/// Mock constructor used by tests and by the state machine before a live server exists.
pub fn server_addr() -> Result<SocketAddr, AppError> {
    Ok(SocketAddr::from(([192, 168, 1, 189], 9091)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_parse_gateway_message_roundtrip() {
        let ev = CaptionEvent {
            segment_id: Uuid::new_v4(),
            start_sample: 0,
            end_sample: 4800,
            text: "안녕하세요".to_string(),
            status: CaptionStatus::Partial,
            speaker_label: Some("화자 1".to_string()),
            overlap: None,
            supersedes: vec![],
        };
        let serialized = serde_json::to_string(&ev).unwrap();
        let parsed = parse_gateway_message(&serialized).unwrap().unwrap();
        assert_eq!(parsed.text, "안녕하세요");
        assert_eq!(parsed.status, CaptionStatus::Partial);
        assert_eq!(parsed.speaker_label.as_deref(), Some("화자 1"));
    }

    #[test]
    fn test_parse_gateway_message_empty() {
        assert!(parse_gateway_message("").unwrap().is_none());
    }
}