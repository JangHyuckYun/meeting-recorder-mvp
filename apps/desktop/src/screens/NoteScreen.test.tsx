import { readFileSync } from "node:fs";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { NoteScreen } from "./NoteScreen";

const noteStyles = readFileSync("src/styles/note.css", "utf8");
const { generateMinutes } = vi.hoisted(() => ({
  generateMinutes: vi.fn().mockResolvedValue({
    recording_id: "rec-1",
    summary: "요약",
    decisions: [],
    action_items: [],
  }),
}));

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
    listTemplates: vi.fn().mockResolvedValue([
      { id: "tpl-1", name: "프로젝트 회의", content: "결정사항\n액션 아이템" },
    ]),
    generateMinutes,
  },
}));

describe("NoteScreen", () => {
  it("renders the recording title, template options, and remaining tabs", async () => {
    render(
      <NoteScreen recordingId="rec-1" onExport={vi.fn()} onAsk={vi.fn()} onShare={vi.fn()} />,
    );

    expect(await screen.findByText("결제 개편 회의")).toBeTruthy();
    expect(screen.getByRole("tab", { name: "한 페이지 문서" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "대화 기록(스크립트)" })).toBeTruthy();
    expect(screen.queryByRole("tab", { name: "스크립트" })).toBeNull();
    expect(screen.getByRole("option", { name: "기본" })).toBeTruthy();
    expect(screen.getByRole("option", { name: "프로젝트 회의" })).toBeTruthy();
  });

  it("passes the selected template to minutes generation", async () => {
    render(
      <NoteScreen recordingId="rec-1" onExport={vi.fn()} onAsk={vi.fn()} onShare={vi.fn()} />,
    );

    const select = await screen.findByRole("combobox", { name: "회의록 템플릿" });
    fireEvent.change(select, { target: { value: "tpl-1" } });
    fireEvent.click(screen.getAllByRole("button", { name: "회의록 생성" })[0]);

    expect(generateMinutes).toHaveBeenCalledWith("rec-1", "tpl-1");
  });

  it("keeps inactive tab panels out of the flex layout", async () => {
    const { container } = render(
      <NoteScreen recordingId="rec-1" onExport={vi.fn()} onAsk={vi.fn()} onShare={vi.fn()} />,
    );

    await within(container).findByText("결제 개편 회의");
    fireEvent.click(within(container).getByRole("tab", { name: "대화 기록(스크립트)" }));

    const panels = container.querySelectorAll<HTMLElement>('[role="tabpanel"]');
    const inactivePanels = [...panels].filter((panel) => panel.hidden);

    expect(inactivePanels).toHaveLength(1);
    for (const panel of inactivePanels) {
      expect(getComputedStyle(panel).display).toBe("none");
    }
    expect(noteStyles).toMatch(
      /\.note-main\s+\[role="tabpanel"\]\[hidden\]\s*\{[^}]*display:\s*none;/s,
    );
  });
});
