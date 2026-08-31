import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { TemplatesScreen } from "./TemplatesScreen";

vi.mock("@/platform/appClient", () => ({
  appClient: {
    listTemplates: vi.fn().mockResolvedValue([]),
    createTemplate: vi.fn(),
    updateTemplate: vi.fn(),
    deleteTemplate: vi.fn(),
  },
}));

afterEach(cleanup);

describe("TemplatesScreen", () => {
  it("rejects an empty template name", async () => {
    render(<TemplatesScreen />);
    fireEvent.click(screen.getByText("+ 나만의 템플릿"));
    fireEvent.click(screen.getByText("템플릿 만들기"));
    await waitFor(() => expect(screen.getByText("템플릿 이름을 입력하세요.")).toBeTruthy());
  });
});
