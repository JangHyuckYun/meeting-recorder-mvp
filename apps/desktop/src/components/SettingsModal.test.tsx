import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SettingsModal } from "./SettingsModal";

vi.mock("@/platform/appClient", () => ({
  appClient: {
    getAppSettings: vi.fn().mockResolvedValue({
      llm_provider: "codex_oauth",
      stt_server_url: null,
      stt_engine: "elevenlabs",
      elevenlabs_api_key_masked: "sk_...ab12",
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
    getGlossary: vi.fn().mockResolvedValue(["더플레이토"]),
    setGlossary: vi.fn().mockResolvedValue(undefined),
  },
}));

afterEach(cleanup);

describe("SettingsModal", () => {
  it("selects ElevenLabs when settings say elevenlabs", async () => {
    render(<SettingsModal open onClose={() => {}} />);
    await waitFor(() => expect(screen.getByTestId("stt-engine-select")).toBeTruthy());
    expect(screen.getByRole("button", { name: /ElevenLabs Scribe/ }).getAttribute("aria-pressed")).toBe("true");
  });

  it("adds a glossary term via the 단어장 tab", async () => {
    render(<SettingsModal open onClose={() => {}} />);

    const glossaryTab = await screen.findByTestId("settings-tab-glossary");
    fireEvent.mouseDown(glossaryTab, { button: 0, ctrlKey: false });
    await waitFor(() => expect(screen.getByText("더플레이토")).toBeTruthy());

    fireEvent.change(screen.getByPlaceholderText("+ 단어 추가"), {
      target: { value: "Scribe" },
    });
    fireEvent.keyDown(screen.getByPlaceholderText("+ 단어 추가"), { key: "Enter" });

    const { appClient } = await import("@/platform/appClient");
    await waitFor(() =>
      expect(appClient.setGlossary).toHaveBeenCalledWith(["더플레이토", "Scribe"]),
    );
  });
});
