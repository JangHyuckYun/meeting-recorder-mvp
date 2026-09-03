import { describe, expect, it } from "vitest";
import { estimateRemainingMs } from "./transcriptionEta";

describe("estimateRemainingMs", () => {
  it("estimates from monotonic progress samples", () => {
    expect(estimateRemainingMs([{ at: 0, fraction: 0.1 }, { at: 10_000, fraction: 0.5 }], 10_000)).toBe(12_500);
  });

  it("waits while progress is stalled", () => {
    expect(estimateRemainingMs([{ at: 0, fraction: 0.01 }], 20_000)).toBeNull();
  });

  it("returns zero on completion", () => {
    expect(estimateRemainingMs([{ at: 0, fraction: 1 }], 0)).toBe(0);
  });
});
