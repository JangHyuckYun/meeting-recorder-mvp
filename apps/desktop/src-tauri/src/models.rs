//! Domain model shared between storage, orchestrator, and the Tauri command surface.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recording {
    pub id: Uuid,
    pub title: String,
    /// Local filesystem path to this recording's audio (wav for live captures, the original
    /// path for ingested test files — never copied into the git repo).
    pub source_path: String,
    pub duration_ms: Option<i64>,
    pub status: RecordingStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingStatus {
    Recording,
    Recorded,
    Transcribing,
    Transcribed,
    MinutesReady,
    Failed,
}

impl RecordingStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RecordingStatus::Recording => "recording",
            RecordingStatus::Recorded => "recorded",
            RecordingStatus::Transcribing => "transcribing",
            RecordingStatus::Transcribed => "transcribed",
            RecordingStatus::MinutesReady => "minutes_ready",
            RecordingStatus::Failed => "failed",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s {
            "recording" => RecordingStatus::Recording,
            "recorded" => RecordingStatus::Recorded,
            "transcribing" => RecordingStatus::Transcribing,
            "transcribed" => RecordingStatus::Transcribed,
            "minutes_ready" => RecordingStatus::MinutesReady,
            _ => RecordingStatus::Failed,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub id: Uuid,
    pub recording_id: Uuid,
    pub start_ms: i64,
    pub end_ms: i64,
    /// Provisional speaker label (e.g. "화자 1") until the user renames it; diarization output
    /// is never treated as a confirmed human identity per `stt-diarization.md`.
    pub speaker_label: String,
    pub text: String,
    pub is_final: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinutesDraft {
    pub recording_id: Uuid,
    pub summary: String,
    pub decisions: Vec<MinutesItem>,
    pub action_items: Vec<MinutesItem>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinutesItem {
    pub id: Uuid,
    pub text: String,
    /// Transcript segment ids this item is grounded in — every decision/action must cite at
    /// least one, mirroring the evidence-ref contract from `llm-minutes.md`.
    pub evidence_segment_ids: Vec<Uuid>,
}

/// Which LLM backend generates and edits meeting minutes. Persisted in `app_settings`; the
/// settings UI switches it at runtime without an app restart.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmProvider {
    #[default]
    /// Self-hosted LiteLLM OpenAI-compatible gateway (192.168.1.189:4000).
    Litellm,
    /// ChatGPT subscription via the Codex CLI's OAuth credentials (~/.codex/auth.json).
    CodexOauth,
    /// Claude subscription via Claude Code's OAuth credentials (~/.claude/.credentials.json).
    ClaudeOauth,
}

impl LlmProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            LlmProvider::Litellm => "litellm",
            LlmProvider::CodexOauth => "codex_oauth",
            LlmProvider::ClaudeOauth => "claude_oauth",
        }
    }

    /// Parses a value previously written by [`LlmProvider::as_str`]. Unknown strings return
    /// `None` so callers can fall back to their default instead of failing on stale data.
    pub fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "litellm" => Some(LlmProvider::Litellm),
            "codex_oauth" => Some(LlmProvider::CodexOauth),
            "claude_oauth" => Some(LlmProvider::ClaudeOauth),
            _ => None,
        }
    }
}

/// User-manageable application settings, served to and persisted from the settings UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub llm_provider: LlmProvider,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            llm_provider: LlmProvider::Litellm,
        }
    }
}
