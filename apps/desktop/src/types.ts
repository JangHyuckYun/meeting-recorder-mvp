export type RecordingStatus =
  | "recording"
  | "recorded"
  | "transcribing"
  | "transcribed"
  | "minutes_ready"
  | "failed";

export interface Recording {
  id: string;
  title: string;
  source_path: string;
  duration_ms: number | null;
  status: RecordingStatus;
  created_at: string;
}

export interface TranscriptSegment {
  id: string;
  recording_id: string;
  start_ms: number;
  end_ms: number;
  speaker_label: string;
  text: string;
  is_final: boolean;
}

export interface MinutesDraft {
  recording_id: string;
  summary: string;
  decisions: MinutesItem[];
  action_items: MinutesItem[];
}

export interface MinutesItem {
  id: string;
  text: string;
  evidence_segment_ids: string[];
}

/// Maps a functional purpose to a registered provider + model name. Frontend uses this
/// to let the user pick which model handles minutes generation vs editing.
export interface ModelAssignment {
  purpose: "minutes_generation" | "minutes_edit";
  provider_id: string;
  model_name: string;
}

export interface ModelAssignmentInput {
  purpose: string;
  provider_id: string;
  model_name: string;
}

/// A registered LLM provider as returned by list_providers.
export interface Provider {
  id: string;
  name: string;
  provider_type: "openai" | "anthropic" | "openai_compatible";
  base_url: string;
  api_key_masked: string;
  models: string[];
  is_active: boolean;
  is_builtin: boolean;
  created_at: string;
}

/// Input payload for adding or updating a provider.
export interface ProviderInput {
  id?: string;
  name: string;
  provider_type: string;
  base_url: string;
  api_key: string;
  models_json: string;
}

/// Legacy types kept for backward compatibility with existing components.
/// New settings UI uses Provider and ModelAssignment instead.
export type LlmProvider = "litellm" | "codex_oauth" | "claude_oauth";

export interface AppSettings {
  llm_provider: LlmProvider;
}

export interface OAuthStatus {
  provider: string;
  logged_in: boolean;
  account_id: string | null;
  expires_at: string | null; // RFC3339 UTC, null when unknown
  access_expired: boolean;
  credentials_path: string;
}
