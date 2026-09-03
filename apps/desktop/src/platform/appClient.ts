/**
 * Single seam between the UI and the Tauri backend. Every `invoke` call the
 * app makes goes through here so screens/components never import
 * `@tauri-apps/api` directly (see grep gate in the shell rebuild plan).
 *
 * Command names/args below mirror `src-tauri/src/commands.rs` exactly.
 * Entries marked STUB are not implemented on the Rust side yet (another
 * agent owns src-tauri) — the shapes are typed ahead of time so screens can
 * be wired against them now.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppSettings,
  CaptionEventData,
  MinutesDraft,
  MinutesItem,
  ModelAssignment,
  ModelAssignmentInput,
  OAuthStatus,
  Provider,
  ProviderInput,
  Recording,
  TranscriptSegment,
} from "@/types";

export interface Folder {
  id: string;
  name: string;
  created_at: string;
}

export interface Template {
  id: string;
  name: string;
  content: string;
}

export type ExportFormat = "srt" | "vtt" | "md" | "txt";

export interface AskSource {
  readonly segment_id: string;
  readonly start_ms: number;
  readonly end_ms: number;
}

export interface AskNoteResult {
  readonly answer: string;
  readonly sources: readonly AskSource[];
}

export interface TranscriptionProgressEvent {
  recording_id: string;
  sent_ms: number;
  total_ms: number;
  phase: "sending" | "finalizing" | "done";
}

export const appClient = {
  // ── Recordings ──────────────────────────────────────────────────────
  listRecordings: () => invoke<Recording[]>("list_recordings"),
  getRecordingDetail: (id: string) =>
    invoke<[Recording, TranscriptSegment[]]>("get_recording_detail", { id }),
  startRecording: (title: string) => invoke<Recording>("start_recording", { title }),
  stopRecording: (captions: CaptionEventData[]) =>
    invoke<Recording>("stop_recording", { captions }),
  ingestAudioFile: (sourcePath: string, title: string) =>
    invoke<Recording>("ingest_audio_file", { sourcePath, title }),
  transcribeRecording: (id: string, speakers?: number | null) =>
    invoke<TranscriptSegment[]>("transcribe_recording", { id, speakers }),
  cancelTranscription: () => invoke<void>("cancel_transcription"),
  updateRecordingStatus: (id: string, status: string) =>
    invoke<void>("update_recording_status", { id, status }),
  deleteRecording: (id: string) => invoke<void>("delete_recording", { id }),
  onTranscriptionProgress: (cb: (event: TranscriptionProgressEvent) => void): Promise<UnlistenFn> =>
    listen<TranscriptionProgressEvent>("transcription-progress", (event) => cb(event.payload)),

  // ── Minutes ─────────────────────────────────────────────────────────
  getMinutes: (recordingId: string) => invoke<MinutesDraft | null>("get_minutes", { recordingId }),
  generateMinutes: (recordingId: string) => invoke<MinutesDraft>("generate_minutes", { recordingId }),
  editMinutesItem: (recordingId: string, itemId: string, instruction: string) =>
    invoke<MinutesItem>("edit_minutes_item", { recordingId, itemId, instruction }),

  // ── Glossary ────────────────────────────────────────────────────────
  getGlossary: () => invoke<string[]>("get_glossary"),
  setGlossary: (terms: string[]) => invoke<void>("set_glossary", { terms }),

  // ── Folders ─────────────────────────────────────────────────────────
  listFolders: () => invoke<Folder[]>("list_folders"),
  createFolder: (name: string) => invoke<Folder>("create_folder", { name }),
  deleteFolder: (id: string) => invoke<void>("delete_folder", { id }),
  assignRecordingFolder: (recordingId: string, folderId: string | null) =>
    invoke<void>("assign_recording_folder", { recordingId, folderId }),

  // ── Templates (STUB — backend not implemented yet) ─────────────────
  listTemplates: () => invoke<Template[]>("list_templates"),
  createTemplate: (name: string, content: string) =>
    invoke<Template>("create_template", { name, content }),
  updateTemplate: (id: string, name: string, content: string) =>
    invoke<void>("update_template", { id, name, content }),
  deleteTemplate: (id: string) => invoke<void>("delete_template", { id }),

  // ── Speaker names (STUB — backend not implemented yet) ─────────────
  getSpeakerNames: (recordingId: string) =>
    invoke<Record<string, string>>("get_speaker_names", { recordingId }),
  setSpeakerName: (recordingId: string, speakerKey: string, name: string) =>
    invoke<void>("set_speaker_name", { recordingId, speakerKey, name }),

  // ── Export / Ask (STUB — backend not implemented yet) ──────────────
  exportTranscript: (recordingId: string, format: ExportFormat) =>
    invoke<string>("export_transcript", { recordingId, format }),
  askNote: (recordingId: string, question: string) =>
    invoke<AskNoteResult>("ask_note", { recordingId, question }),

  // ── Settings / Providers ────────────────────────────────────────────
  getAppSettings: () => invoke<AppSettings>("get_app_settings"),
  setAppSettings: (settings: AppSettings) => invoke<void>("set_app_settings", { settings }),
  setElevenLabsApiKey: (apiKey: string) => invoke<void>("set_elevenlabs_api_key", { apiKey }),
  listProviders: () => invoke<Provider[]>("list_providers"),
  addProvider: (input: ProviderInput) => invoke<string>("add_provider", { input }),
  updateProvider: (input: ProviderInput) => invoke<void>("update_provider", { input }),
  deleteProvider: (id: string) => invoke<void>("delete_provider", { id }),
  getModelAssignments: () => invoke<ModelAssignment[]>("get_model_assignments"),
  setModelAssignment: (input: ModelAssignmentInput) => invoke<void>("set_model_assignment", { input }),
  listRemoteModels: (providerId: string) => invoke<string[]>("list_remote_models", { providerId }),
  getOAuthStatus: (provider: string) => invoke<OAuthStatus>("get_oauth_status", { provider }),
  startOAuthLogin: (provider: "openai" | "anthropic") => invoke<{ authorize_url: string }>("start_oauth_login", { provider }),
  completeOAuthLogin: (provider: "openai" | "anthropic", codeOrRedirectUrl?: string) => invoke<OAuthStatus>("complete_oauth_login", { provider, code_or_redirect_url: codeOrRedirectUrl }),
};

export type AppClient = typeof appClient;
