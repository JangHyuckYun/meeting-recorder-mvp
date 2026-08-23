/**
 * Canvas geometry constants.
 *
 * Canvas paints pixels directly, so it cannot use Tailwind spacing utilities.
 * Every value below is a named multiple of the same 4px grid the CSS scale uses
 * (see DESIGN.md §Spacing). No canvas component may inline a raw number.
 */

/** Base grid unit, matching Tailwind's `1` = 0.25rem = 4px. */
export const GRID = 4;

export const WAVE = {
  /** Default surface height: 21 grid units. */
  height: GRID * 21,
  barWidth: GRID * 0.75,
  barGap: GRID * 0.5,
  minBarHeight: GRID * 0.5,
  baselineWidth: 1,
  cornerRadius: GRID * 0.375,
  /** Horizontal inset so bars never touch the panel edge. */
  insetX: GRID,
} as const;

export const TIMELINE = {
  height: GRID * 14,
  rulerHeight: GRID * 5,
  laneHeight: GRID * 5,
  laneRadius: GRID * 0.75,
  tickLength: GRID,
  labelOffset: GRID * 0.75,
  cursorWidth: 2,
  insetX: GRID * 2,
  fontSize: 10,
} as const;

export const TRACKS = {
  gutterWidth: GRID * 22,
  laneHeight: GRID * 7,
  laneGap: GRID * 2,
  laneRadius: GRID,
  segmentInset: GRID * 0.5,
  paddingY: GRID * 2,
  insetX: GRID * 2,
  fontSize: 11,
  overlapStripe: GRID,
} as const;

/** Tick intervals in ms, ordered; the smallest one yielding readable spacing wins. */
export const TICK_STEPS_MS = [
  1_000, 2_000, 5_000, 10_000, 15_000, 30_000, 60_000, 120_000, 300_000, 600_000, 900_000,
  1_800_000, 3_600_000,
] as const;

/** Chooses a tick interval so labels stay at least `minPx` apart. */
export function pickTickStep(durationMs: number, widthPx: number, minPx: number): number {
  const lastStep = TICK_STEPS_MS[TICK_STEPS_MS.length - 1] ?? 3_600_000;
  if (durationMs <= 0 || widthPx <= 0) return lastStep;
  for (const step of TICK_STEPS_MS) {
    if ((step / durationMs) * widthPx >= minPx) return step;
  }
  return lastStep;
}

/** Compact axis label: `m:ss` under an hour, `h:mm:ss` beyond it. */
export function formatTick(ms: number): string {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  const seconds = totalSeconds % 60;
  const minutes = Math.floor(totalSeconds / 60) % 60;
  const hours = Math.floor(totalSeconds / 3600);
  const paddedSeconds = String(seconds).padStart(2, "0");
  return hours > 0
    ? `${hours}:${String(minutes).padStart(2, "0")}:${paddedSeconds}`
    : `${minutes}:${paddedSeconds}`;
}

/** Rounded rectangle path helper (Path2D roundRect is not universally available). */
export function roundedRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  width: number,
  height: number,
  radius: number,
): void {
  const r = Math.max(0, Math.min(radius, width / 2, height / 2));
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.lineTo(x + width - r, y);
  ctx.quadraticCurveTo(x + width, y, x + width, y + r);
  ctx.lineTo(x + width, y + height - r);
  ctx.quadraticCurveTo(x + width, y + height, x + width - r, y + height);
  ctx.lineTo(x + r, y + height);
  ctx.quadraticCurveTo(x, y + height, x, y + height - r);
  ctx.lineTo(x, y + r);
  ctx.quadraticCurveTo(x, y, x + r, y);
  ctx.closePath();
}
