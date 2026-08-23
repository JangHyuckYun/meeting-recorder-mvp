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
