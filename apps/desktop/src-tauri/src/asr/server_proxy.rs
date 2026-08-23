//! ASR v2 gateway client. WebSocket transport to the on-LAN ASR gateway (port 9091),
//! sends stereo-downmixed mono 16kHz PCM chunks and receives CaptionEvent JSON.
//!
//! NOTE: The state machine lives in `asr/live_stt.rs`. This module owns the transport.

use crate::asr::domain::parse_gateway_message;
use crate::error::{AppError, AppResult};
use crate::models::CaptionEvent;
use std::time::Duration;
use tokio_tungstenite::connect_async;

/// Gateway connection parameters.
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    pub host: String,
    pub port: u16,
    pub max_reconnect_retries: u32,
    pub reconnect_delay: Duration,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            host: "192.168.1.189".to_string(),
            port: 9091,
            max_reconnect_retries: 5,
            reconnect_delay: Duration::from_secs(2),
        }
    }
}

/// Establish a WebSocket connection to the ASR gateway.
pub async fn connect(cfg: &GatewayConfig) -> AppResult<WebSocketTcpConnection> {
    let url = format!("ws://{}/asr", cfg.host);
    let addr = format!("{}:{}", cfg.host, cfg.port);
    let (socket, _) = connect_async(addr)
        .await
        .map_err(|e| AppError::Stt(format!("gateway connect failed: {e}")))?;
    let _ = url;
    Ok(WebSocketTcpConnection { socket })
}

/// Thin wrapper around a connected gateway WebSocket.
pub struct WebSocketTcpConnection {
    socket: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
}

use futures_util::{SinkExt, StreamExt};

impl WebSocketTcpConnection {
    /// Send a mono 16kHz f32 chunk to the gateway.
    pub async fn send_mono_pcm(&mut self, samples: &[f32]) -> AppResult<()> {
        let bytes: Vec<u8> = samples
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        let msg = tokio_tungstenite::tungstenite::Message::Binary(bytes);
        self.socket
            .send(msg)
            .await
            .map_err(|e| AppError::Stt(format!("gateway send failed: {e}")))?;
        Ok(())
    }

    /// Read the next message from the gateway. Returns None on close/error.
    pub async fn next_caption(&mut self) -> AppResult<Option<CaptionEvent>> {
        loop {
            match self.socket.next().await {
                Some(Ok(msg)) => match msg {
                    tokio_tungstenite::tungstenite::Message::Text(text) => {
                        return parse_gateway_message(&text);
                    }
                    tokio_tungstenite::tungstenite::Message::Binary(bin) => {
                        let raw = String::from_utf8_lossy(&bin);
                        return parse_gateway_message(&raw);
                    }
                    _ => continue,
                },
                Some(Err(e)) => {
                    return Err(AppError::Stt(format!("gateway read error: {e}")));
                }
                None => return Ok(None),
            }
        }
    }

    /// Send a done/stop marker to finish the stream.
    pub async fn send_done(&mut self) -> AppResult<()> {
        let msg = tokio_tungstenite::tungstenite::Message::Text("Done".to_string());
        self.socket
            .send(msg)
            .await
            .map_err(|e| AppError::Stt(format!("gateway done send failed: {e}")))?;
        Ok(())
    }
}