import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { ROUTES } from "./routes";

vi.mock("@/platform/appClient", () => ({
  appClient: {
    listRecordings: vi.fn().mockResolvedValue([]),
    getRecordingDetail: vi.fn().mockResolvedValue([{}, []]),
    startRecording: vi.fn(),
    stopRecording: vi.fn(),
    ingestAudioFile: vi.fn(),
    transcribeRecording: vi.fn(),
    cancelTranscription: vi.fn(),
    updateRecordingStatus: vi.fn(),
    deleteRecording: vi.fn(),
    onTranscriptionProgress: vi.fn().mockResolvedValue(() => {}),
    getMinutes: vi.fn().mockResolvedValue(null),
    generateMinutes: vi.fn(),
    editMinutesItem: vi.fn(),
    getGlossary: vi.fn().mockResolvedValue([]),
    setGlossary: vi.fn(),
    listFolders: vi.fn().mockResolvedValue([]),
    createFolder: vi.fn(),
    deleteFolder: vi.fn(),
    assignRecordingFolder: vi.fn(),
    listTemplates: vi.fn().mockResolvedValue([]),
    createTemplate: vi.fn(),
    updateTemplate: vi.fn(),
    deleteTemplate: vi.fn(),
    getSpeakerNames: vi.fn().mockResolvedValue({}),
    setSpeakerName: vi.fn(),
    exportTranscript: vi.fn(),
    askNote: vi.fn(),
    getAppSettings: vi.fn().mockResolvedValue({
      llm_provider: "codex_oauth",
      stt_server_url: null,
      stt_engine: "self_hosted",
      elevenlabs_api_key_masked: null,
    }),
    setAppSettings: vi.fn(),
    setElevenLabsApiKey: vi.fn(),
    listProviders: vi.fn().mockResolvedValue([]),
    addProvider: vi.fn(),
    deleteProvider: vi.fn(),
    getModelAssignments: vi.fn().mockResolvedValue([]),
    setModelAssignment: vi.fn(),
    getOAuthStatus: vi.fn().mockResolvedValue({
      provider: "codex_oauth",
      logged_in: false,
      account_id: null,
      expires_at: null,
      access_expired: false,
      credentials_path: "",
    }),
  },
}));

afterEach(cleanup);

describe("S1-S10 router", () => {
  it.each(ROUTES)("renders the %s screen container", (route) => {
    render(<App initialRoute={route} />);
    expect(screen.getByTestId(`screen-${route}`)).toBeTruthy();
  });
});
