import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AskScreen } from "./AskScreen";

vi.mock("@/platform/appClient", () => ({
  appClient: {
    askNote: vi.fn().mockResolvedValue({ answer: "결정사항은 A입니다.", sources: ["seg-1"] }),
  },
}));

afterEach(cleanup);

describe("AskScreen", () => {
  it("renders the answer with a source chip", async () => {
    render(<AskScreen recordingId="rec-1" />);
    fireEvent.change(screen.getByPlaceholderText("이 노트에 대해 무엇이든 물어보세요"), {
      target: { value: "결정사항은?" },
    });
    fireEvent.click(screen.getByRole("button", { name: "전송" }));
    await waitFor(() => expect(screen.getByText("결정사항은 A입니다.")).toBeTruthy());
    expect(screen.getByText(/seg-1/)).toBeTruthy();
  });
});
