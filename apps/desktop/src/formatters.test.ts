import { describe, expect, it } from "vitest";
import { formatDuration } from "./formatters";

describe("formatDuration", () => {
  it("renders mm:ss for durations under an hour", () => {
    expect(formatDuration(65_000)).toBe("01:05");
  });

  it("renders hh:mm:ss once an hour is crossed", () => {
    expect(formatDuration(3_661_000)).toBe("01:01:01");
  });

  it("renders a placeholder for unknown duration", () => {
    expect(formatDuration(null)).toBe("--:--");
  });
});
