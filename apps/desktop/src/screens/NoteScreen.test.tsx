import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { NoteScreen } from "./NoteScreen";

vi.mock("@/platform/appClient", () => ({
  appClient: {
    getRecordingDetail: vi.fn().mockResolvedValue([
      {
        id: "rec-1",
        title: "결제 개편 회의",
        source_path: "",
        duration_ms: 60_000,
        status: "minutes_ready",
        created_at: "2026-06-20T14:40:00Z",
        folder_id: null,
      },
      [
        {
          id: "seg-1",
          recording_id: "rec-1",
          start_ms: 0,
          end_ms: 1000,
          speaker_label: "SPEAKER_0",
          text: "안녕하세요",
          is_final: true,
        },
      ],
    ]),
    getMinutes: vi.fn().mockResolvedValue(null),
    getSpeakerNames: vi.fn().mockResolvedValue({}),
  },
}));

describe("NoteScreen", () => {
  it("renders the recording title and 3 view tabs", async () => {
    render(
      <NoteScreen recordingId="rec-1" onExport={vi.fn()} onAsk={vi.fn()} onShare={vi.fn()} />,
    );

    expect(await screen.findByText("결제 개편 회의")).toBeTruthy();
    expect(screen.getByRole("tab", { name: "한 페이지 문서" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "대화 기록" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "스크립트" })).toBeTruthy();
  });
});
