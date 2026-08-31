import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { LiveRecordingScreen } from "./LiveRecordingScreen";

// jsdom implements neither matchMedia nor ResizeObserver; the canvas
// components query prefers-reduced-motion and observe resize on mount.
beforeAll(() => {
  window.matchMedia ??= ((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: () => {},
    removeListener: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
    dispatchEvent: () => false,
  })) as unknown as typeof window.matchMedia;

  (globalThis as { ResizeObserver?: unknown }).ResizeObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
});

vi.mock("@/platform/appClient", () => ({
  appClient: {
    startRecording: vi.fn().mockResolvedValue({
      id: "rec-1",
      title: "새 회의",
      source_path: "",
      duration_ms: null,
      status: "recording",
      created_at: "",
    }),
    stopRecording: vi.fn(),
  },
}));

vi.mock("@/hooks/useCaptionEvents", () => ({
  useCaptionEvents: () => ({
    segments: new Map([
      [
        "seg-1",
        {
          segment_id: "seg-1",
          start_sample: 0,
          end_sample: 16_000,
          text: "안녕하세요",
          status: "committed",
          speaker_label: "SPEAKER_1",
        },
      ],
    ]),
    sequence: 1,
  }),
}));

describe("LiveRecordingScreen", () => {
  it("starts recording and renders the live transcript line", async () => {
    render(<LiveRecordingScreen />);

    fireEvent.click(screen.getByRole("button", { name: /녹음 시작/ }));

    await waitFor(() => screen.getByText(/녹음 중/));
    expect(screen.getByText("안녕하세요")).toBeTruthy();
  });
});
