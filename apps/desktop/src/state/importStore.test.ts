import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Recording } from "@/types";
import { importStore, useImportJob } from "./importStore";

const recording: Recording = {
  id: "rec-1", title: "meeting.m4a", source_path: "", duration_ms: 60_000,
  status: "recorded", created_at: "", folder_id: null,
};

vi.mock("@/platform/appClient", () => ({
  appClient: {
    getActiveTranscriptions: vi.fn().mockResolvedValue([]),
    listRecordings: vi.fn().mockResolvedValue([]),
    onTranscriptionProgress: vi.fn().mockResolvedValue(() => {}),
  },
}));

describe("importStore", () => {
  beforeEach(() => importStore.reset());

  it("keeps the active job across hook unmount and remount", () => {
    const first = renderHook(() => useImportJob());
    act(() => importStore.start(recording));
    first.unmount();
    const second = renderHook(() => useImportJob());
    expect(second.result.current?.recording.title).toBe("meeting.m4a");
  });

  it("hydrates an active backend job", async () => {
    const { appClient } = await import("@/platform/appClient");
    vi.mocked(appClient.getActiveTranscriptions).mockResolvedValueOnce([
      { recording_id: "rec-1", sent_ms: 30_000, total_ms: 60_000, phase: "sending" },
    ]);
    vi.mocked(appClient.listRecordings).mockResolvedValueOnce([recording]);
    await importStore.hydrate();
    expect(importStore.getSnapshot()?.progress.sent_ms).toBe(30_000);
  });
});
