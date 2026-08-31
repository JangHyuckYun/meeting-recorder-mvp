import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ExportScreen } from "./ExportScreen";

vi.mock("@/platform/appClient", () => ({
  appClient: {
    exportTranscript: vi.fn().mockResolvedValue("/Users/me/Downloads/result.srt"),
  },
}));

afterEach(cleanup);

describe("ExportScreen", () => {
  it("shows the written path after export", async () => {
    render(<ExportScreen recordingId="rec-1" />);
    fireEvent.click(screen.getByRole("button", { name: "내보내기" }));
    await waitFor(() => expect(screen.getByText(/result\.srt/)).toBeTruthy());
  });
});
