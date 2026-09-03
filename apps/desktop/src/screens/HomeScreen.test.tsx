import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { HomeScreen } from "./HomeScreen";

vi.mock("@/platform/appClient", () => ({
  appClient: {
    getActiveTranscriptions: vi.fn().mockResolvedValue([]),
    onTranscriptionProgress: vi.fn().mockResolvedValue(() => {}),
    listRecordings: vi.fn().mockResolvedValue([
      {
        id: "rec-1",
        title: "결제 개편 회의",
        source_path: "",
        duration_ms: 60_000,
        status: "minutes_ready",
        created_at: "2026-06-20T14:40:00Z",
        folder_id: null,
      },
    ]),
    listFolders: vi.fn().mockResolvedValue([]),
    createFolder: vi.fn(),
    assignRecordingFolder: vi.fn(),
    deleteRecording: vi.fn(),
  },
}));

describe("HomeScreen", () => {
  it("lists recordings and opens a note on row click", async () => {
    const onOpenNote = vi.fn();
    render(<HomeScreen onOpenNote={onOpenNote} />);

    const row = await screen.findByText("결제 개편 회의");
    fireEvent.click(row);

    expect(onOpenNote).toHaveBeenCalledWith("rec-1");
  });

  it("redirects to import while transcription is active", async () => {
    const { appClient } = await import("@/platform/appClient");
    vi.mocked(appClient.getActiveTranscriptions).mockResolvedValueOnce([
      { recording_id: "rec-1", sent_ms: 30_000, total_ms: 60_000, phase: "sending" },
    ]);
    const onOpenNote = vi.fn();
    const onOpenImport = vi.fn();
    render(<HomeScreen onOpenNote={onOpenNote} onOpenImport={onOpenImport} />);
    expect((await screen.findAllByText(/전사 진행 중/)).length).toBeGreaterThan(0);
    expect(onOpenImport).toHaveBeenCalled();
  });
});
