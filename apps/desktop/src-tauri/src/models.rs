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
    /// Folder this recording is filed under, or `None` for the unfiled root list.
    #[serde(default)]
    pub folder_id: Option<Uuid>,
}

/// A user-created folder grouping recordings in the sidebar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

/// A user-authored minutes template. `content` is appended to the minutes system prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub id: Uuid,
    pub name: String,
    pub content: String,
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

/// Answer to a grounded question about one recording, with the transcript segments the model
/// cited. Sources are best-effort: an answer the model failed to cite still returns cleanly
/// with an empty `sources`, never an error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskAnswer {
    pub answer: String,
    pub sources: Vec<AskSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AskSource {
    pub segment_id: Uuid,
    pub start_ms: i64,
    pub end_ms: i64,
}

/// 48kHz sample index as the canonical clock for audio timing.
/// Milliseconds are presentation values; the sample index is the source of truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AudioTime(pub i64);

impl AudioTime {
    pub const SAMPLE_RATE: i64 = 48_000;

    pub fn from_samples(samples: i64) -> Self {
        Self(samples)
    }

    pub fn from_ms(ms: i64) -> Self {
        Self(ms * Self::SAMPLE_RATE / 1000)
    }

    pub fn as_samples(self) -> i64 {
        self.0
    }

    pub fn as_ms(self) -> i64 {
        self.0 * 1000 / Self::SAMPLE_RATE
    }

    pub fn diff(self, other: Self) -> i64 {
        self.0 - other.0
    }

    pub fn add_samples(self, samples: i64) -> Self {
        Self(self.0 + samples)
    }

    pub fn add_ms(self, ms: i64) -> Self {
        Self(self.0 + ms * Self::SAMPLE_RATE / 1000)
    }
}

/// Caption segment lifecycle: live caption may transition PARTIAL→STABLE→COMMITTED→REVISED.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptionStatus {
    /// Live hypothesis from Zipformer — replaceable, never persisted.
    Absent,
    /// Streaming partial from Zipformer, replaceable in frontend buffer.
    Partial,
    /// Zipformer endpoint-sealed; frontend freezes this segment.
    Stable,
    /// Turbo correction accepted; consumed by minutes pipeline.
    Committed,
    /// Overlap separation or offline replacement supersedes a committed segment.
    Revised,
}

/// Overlap status for a caption event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlapInfo {
    pub speaker_count: u32,
    pub retired_label: Option<String>,
}

/// A single caption event emitted by the ASR pipeline and consumed by the frontend.
/// The canonical clock is `start_sample` / `end_sample` in AudioTime (48kHz sample index).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptionEvent {
    pub segment_id: Uuid,
    pub start_sample: i64,
    pub end_sample: i64,
    pub text: String,
    pub status: CaptionStatus,
    pub speaker_label: Option<String>,
    pub overlap: Option<OverlapInfo>,
    /// Segment IDs this revision replaces (for REVISED events).
    pub supersedes: Vec<Uuid>,
}

impl CaptionEvent {
    pub fn duration_ms(&self) -> i64 {
        AudioTime::from_samples(self.end_sample - self.start_sample).as_ms()
    }

    pub fn start_ms(&self) -> i64 {
        AudioTime::from_samples(self.start_sample).as_ms()
    }

    pub fn end_ms(&self) -> i64 {
        AudioTime::from_samples(self.end_sample).as_ms()
    }
}

/// Which STT backend transcribes recordings. Persisted in `app_settings`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SttEngine {
    #[default]
    SelfHosted,
    Elevenlabs,
}

impl SttEngine {
    pub fn as_str(&self) -> &'static str {
        match self {
            SttEngine::SelfHosted => "self_hosted",
            SttEngine::Elevenlabs => "elevenlabs",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "self_hosted" => Some(SttEngine::SelfHosted),
            "elevenlabs" => Some(SttEngine::Elevenlabs),
            _ => None,
        }
    }
}

/// Which LLM backend generates and edits meeting minutes. Persisted in `app_settings`.
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

/// A registered LLM provider endpoint. Built-in providers (codex_oauth, claude_oauth) are
/// seeded by migration and cannot be deleted; user-added providers (OpenAI key, vLLM, etc.)
/// can be freely managed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: Uuid,
    pub name: String,
    pub provider_type: ProviderType,
    /// API base URL. For built-in OAuth providers this is empty; for registered providers it
    /// is the endpoint (e.g. https://api.openai.com/v1).
    pub base_url: String,
    /// Stored API key (masked when sent to the frontend). Empty for OAuth providers.
    pub api_key_masked: String,
    /// Available models as a JSON array string (e.g. `["gpt-4o","gpt-4.1-mini"]`).
    pub models_json: String,
    pub is_active: bool,
    pub is_builtin: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    /// api.openai.com / compatible with OpenAI chat completions schema
    Openai,
    /// api.anthropic.com / Anthropic Messages API
    Anthropic,
    /// Any OpenAI-compatible endpoint (vLLM, Ollama, LiteLLM gateway, etc.)
    OpenaiCompatible,
}

impl ProviderType {
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "openai" => Some(ProviderType::Openai),
            "anthropic" => Some(ProviderType::Anthropic),
            "openai_compatible" => Some(ProviderType::OpenaiCompatible),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ProviderType::Openai => "openai",
            ProviderType::Anthropic => "anthropic",
            ProviderType::OpenaiCompatible => "openai_compatible",
        }
    }
}

/// Maps a functional purpose to a registered provider + model name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelAssignment {
    pub purpose: ModelPurpose,
    pub provider_id: Uuid,
    /// The actual model name (e.g. "gpt-4o", "claude-sonnet-4-20250514").
    pub model_name: String,
    pub reasoning_effort: Option<String>,
    pub fast: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ModelPurpose {
    #[default]
    MinutesGeneration,
    MinutesEdit,
}

impl ModelPurpose {
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "minutes_generation" => Some(ModelPurpose::MinutesGeneration),
            "minutes_edit" => Some(ModelPurpose::MinutesEdit),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ModelPurpose::MinutesGeneration => "minutes_generation",
            ModelPurpose::MinutesEdit => "minutes_edit",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ModelPurpose::MinutesGeneration => "회의록 생성",
            ModelPurpose::MinutesEdit => "회의록 항목 수정",
        }
    }
}

/// Payload for adding or updating a provider from the frontend. The `api_key` field is
/// always unmasked in requests; responses carry it masked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInput {
    pub id: Option<String>,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub api_key: String,
    pub models_json: String,
}

/// Payload for setting a model assignment from the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelAssignmentInput {
    pub purpose: String,
    pub provider_id: String,
    pub model_name: String,
    pub reasoning_effort: Option<String>,
    pub fast: bool,
}

/// Lightweight DTO for the frontend provider list (no api_key, no models_json full).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSummary {
    pub id: Uuid,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub api_key_masked: String,
    pub models: Vec<String>,
    pub is_active: bool,
    pub is_builtin: bool,
}

/// User-manageable application settings, served to and persisted from the settings UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub llm_provider: LlmProvider,
    pub stt_server_url: Option<String>,
    pub stt_engine: SttEngine,
    pub speakers: Option<u32>,
    pub elevenlabs_api_key_masked: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            llm_provider: LlmProvider::Litellm,
            stt_server_url: Some("ws://192.168.1.189:9090".to_string()),
            stt_engine: SttEngine::SelfHosted,
            speakers: None,
            elevenlabs_api_key_masked: None,
        }
    }
}
