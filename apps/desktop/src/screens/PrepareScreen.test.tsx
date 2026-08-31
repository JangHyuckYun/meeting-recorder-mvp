import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { PrepareScreen } from "./PrepareScreen";

vi.mock("@/platform/appClient", () => ({
  appClient: {
    getGlossary: vi.fn().mockResolvedValue(["박서연", "재시도정책"]),
    setGlossary: vi.fn().mockResolvedValue(undefined),
    startRecording: vi.fn().mockResolvedValue({
      id: "rec-1",
      title: "회의",
      source_path: "",
      duration_ms: null,
      status: "recording",
      created_at: "",
    }),
  },
}));

describe("PrepareScreen", () => {
  it("starts recording and calls onStart", async () => {
    const onStart = vi.fn();
    render(<PrepareScreen onStart={onStart} />);

    fireEvent.click(screen.getByRole("button", { name: /기록 시작/ }));

    await waitFor(() => expect(onStart).toHaveBeenCalledTimes(1));
  });
});
