import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ImportScreen } from "./ImportScreen";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn().mockResolvedValue("/tmp/meeting.m4a"),
}));

vi.mock("@/platform/appClient", () => ({
  appClient: {
    ingestAudioFile: vi.fn().mockResolvedValue({
      id: "rec-1",
      title: "meeting",
      source_path: "/tmp/meeting.m4a",
      duration_ms: null,
      status: "recorded",
      created_at: "",
    }),
    transcribeRecording: vi.fn().mockResolvedValue([]),
    cancelTranscription: vi.fn(),
    onTranscriptionProgress: vi.fn().mockResolvedValue(() => {}),
    getAppSettings: vi.fn().mockResolvedValue({ speakers: null }),
  },
}));

describe("ImportScreen", () => {
  it("imports a picked file and offers to start transcription", async () => {
    render(<ImportScreen />);

    fireEvent.click(screen.getByRole("button", { name: "오디오 파일" }));
    await waitFor(() => {
      expect(document.getElementById("import-file-path")?.textContent).toBe("meeting.m4a");
    });

    fireEvent.click(screen.getByRole("button", { name: "가져오기" }));
    await waitFor(() => screen.getByRole("button", { name: "전사 시작" }));

    fireEvent.click(screen.getByRole("button", { name: "전사 시작" }));
    const { appClient } = await import("@/platform/appClient");
    await waitFor(() => expect(appClient.transcribeRecording).toHaveBeenCalledWith("rec-1", null));
  });
});
