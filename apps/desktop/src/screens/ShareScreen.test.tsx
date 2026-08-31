import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { ShareScreen } from "./ShareScreen";

afterEach(cleanup);

describe("ShareScreen", () => {
  it("shows the local-preview notice and adds a local invitee", () => {
    render(<ShareScreen recordingId="rec-1" />);
    expect(screen.getByText(/공유 서버 준비 중/)).toBeTruthy();

    fireEvent.change(screen.getByPlaceholderText("이름 또는 이메일로 초대"), {
      target: { value: "seoyeon@theplato.io" },
    });
    fireEvent.click(screen.getByRole("button", { name: "초대" }));
    expect(screen.getByText("초대된 사람 (1)")).toBeTruthy();
  });
});
