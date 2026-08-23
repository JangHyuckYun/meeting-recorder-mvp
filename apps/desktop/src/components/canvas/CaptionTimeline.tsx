import { useCallback, useEffect, type KeyboardEvent, type PointerEvent } from "react";

import { hsl, speakerColor, type CaptionState } from "@/lib/design-tokens";
import { cn } from "@/lib/utils";

import { GRID, TIMELINE, formatTick, pickTickStep, roundedRect } from "./geometry";
import type { CaptionSpan } from "./types";
import { useCanvas, type CanvasSurface } from "./use-canvas";

/** Fill opacity per lifecycle state: a hypothesis must never look committed. */
const STATE_ALPHA: Record<CaptionState, number> = {
  partial: 0.2,
  stable: 0.45,
  committed: 0.8,
  revised: 0.9,
};

const MIN_LABEL_SPACING = GRID * 16;
const MIN_BLOCK_WIDTH = GRID * 0.75;
const SEEK_STEP_MS = 1_000;
const SEEK_STEP_COARSE_MS = 10_000;

export interface CaptionTimelineProps {
  captions?: readonly CaptionSpan[];
  /** Total recording length in ms; drives the axis scale. */
  durationMs: number;
  /** Playhead position in ms. */
  cursorMs?: number | null;
  /** Enables seeking by click and keyboard when provided. */
  onSeek?: (positionMs: number) => void;
  height?: number;
  label?: string;
  /** Message painted when there is nothing to show yet. */
  emptyLabel?: string;
  className?: string;
}

/**
 * Time axis of caption blocks, colored by speaker and shaded by lifecycle state.
 * Renders exactly the captions supplied — an empty session stays empty.
 */
export function CaptionTimeline({
  captions = [],
  durationMs,
  cursorMs = null,
  onSeek,
  height = TIMELINE.height,
  label = "자막 타임라인",
  emptyLabel = "자막 대기 중",
  className,
}: CaptionTimelineProps) {
  const seekable = typeof onSeek === "function";

  const paint = useCallback(
    ({ ctx, width, height: surfaceHeight, palette }: CanvasSurface) => {
      const usableWidth = Math.max(0, width - TIMELINE.insetX * 2);
      if (usableWidth <= 0) return;

      const scale = durationMs > 0 ? usableWidth / durationMs : 0;
      const toX = (ms: number) =>
        TIMELINE.insetX + Math.min(usableWidth, Math.max(0, ms * scale));
      const laneTop = TIMELINE.rulerHeight;
      const laneHeight = Math.max(TIMELINE.laneHeight, surfaceHeight - TIMELINE.rulerHeight - GRID);

      // --- ruler -------------------------------------------------------
      ctx.fillStyle = hsl(palette.mutedForeground);
      ctx.font = `${TIMELINE.fontSize}px ${palette.fontMono}`;
      ctx.textBaseline = "top";
      ctx.textAlign = "left";

      if (durationMs > 0) {
        const step = pickTickStep(durationMs, usableWidth, MIN_LABEL_SPACING);
        for (let tick = 0; tick <= durationMs; tick += step) {
          const x = toX(tick);
          ctx.fillStyle = hsl(palette.waveGrid);
          ctx.fillRect(x, laneTop - TIMELINE.tickLength, 1, TIMELINE.tickLength);
          ctx.fillStyle = hsl(palette.subtleForeground);
          ctx.fillText(formatTick(tick), x + TIMELINE.labelOffset, GRID * 0.5);
        }
      }

      // --- lane background --------------------------------------------
      ctx.fillStyle = hsl(palette.waveBaseline, 0.6);
      roundedRect(ctx, TIMELINE.insetX, laneTop, usableWidth, laneHeight, TIMELINE.laneRadius);
      ctx.fill();

      if (captions.length === 0 || durationMs <= 0) {
        ctx.fillStyle = hsl(palette.subtleForeground);
        ctx.font = `${TIMELINE.fontSize + 1}px ${palette.fontSans}`;
        ctx.textAlign = "center";
        ctx.textBaseline = "middle";
        ctx.fillText(emptyLabel, TIMELINE.insetX + usableWidth / 2, laneTop + laneHeight / 2);
        return;
      }

      // --- caption blocks ----------------------------------------------
      for (const caption of captions) {
        const startX = toX(caption.startMs);
        const endX = toX(Math.max(caption.startMs, caption.endMs));
        const blockWidth = Math.max(MIN_BLOCK_WIDTH, endX - startX);
        const base =
          caption.speakerIndex === null || caption.speakerIndex === undefined
            ? palette.state[caption.state]
            : speakerColor(palette, caption.speakerIndex);

        ctx.fillStyle = hsl(base, STATE_ALPHA[caption.state]);
        roundedRect(ctx, startX, laneTop, blockWidth, laneHeight, TIMELINE.laneRadius);
        ctx.fill();

        if (caption.state === "partial") {
          ctx.save();
          ctx.setLineDash([GRID * 0.75, GRID * 0.75]);
          ctx.strokeStyle = hsl(palette.state.partial);
          ctx.lineWidth = 1;
          roundedRect(
            ctx,
            startX + 0.5,
            laneTop + 0.5,
            Math.max(MIN_BLOCK_WIDTH, blockWidth - 1),
            laneHeight - 1,
            TIMELINE.laneRadius,
          );
          ctx.stroke();
          ctx.restore();
        }

        if (caption.overlap) {
          ctx.fillStyle = hsl(palette.waveOverlap);
          ctx.fillRect(startX, laneTop, blockWidth, GRID * 0.5);
        }
      }

      // --- playhead -----------------------------------------------------
      if (cursorMs !== null && cursorMs !== undefined && durationMs > 0) {
        const x = toX(cursorMs);
        ctx.fillStyle = hsl(palette.waveCursor);
        ctx.fillRect(x - TIMELINE.cursorWidth / 2, laneTop - GRID, TIMELINE.cursorWidth, laneHeight + GRID);
      }
    },
    [captions, cursorMs, durationMs, emptyLabel],
  );

  const { canvasRef, repaint } = useCanvas(paint);

  useEffect(() => {
    repaint();
  }, [captions, cursorMs, durationMs, repaint]);

  const seekFromPointer = (event: PointerEvent<HTMLCanvasElement>) => {
    if (!onSeek || durationMs <= 0) return;
    const rect = event.currentTarget.getBoundingClientRect();
    const usableWidth = Math.max(1, rect.width - TIMELINE.insetX * 2);
    const ratio = (event.clientX - rect.left - TIMELINE.insetX) / usableWidth;
    onSeek(Math.round(Math.min(1, Math.max(0, ratio)) * durationMs));
  };

  const seekFromKey = (event: KeyboardEvent<HTMLCanvasElement>) => {
    if (!onSeek || durationMs <= 0) return;
    const current = cursorMs ?? 0;
    const step = event.shiftKey ? SEEK_STEP_COARSE_MS : SEEK_STEP_MS;
    const next =
      event.key === "ArrowLeft"
        ? current - step
        : event.key === "ArrowRight"
          ? current + step
          : event.key === "Home"
            ? 0
            : event.key === "End"
              ? durationMs
              : null;
    if (next === null) return;
    event.preventDefault();
    onSeek(Math.round(Math.min(durationMs, Math.max(0, next))));
  };

  const interactionProps = seekable
    ? ({
        role: "slider",
        tabIndex: 0,
        "aria-valuemin": 0,
        "aria-valuemax": durationMs,
        "aria-valuenow": cursorMs ?? 0,
        "aria-valuetext": formatTick(cursorMs ?? 0),
        onPointerDown: seekFromPointer,
        onKeyDown: seekFromKey,
      } as const)
    : ({ role: "img" } as const);

  return (
    <canvas
      ref={canvasRef}
      aria-label={label}
      className={cn(
        "block w-full rounded-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background",
        seekable && "cursor-pointer",
        className,
      )}
      style={{ height }}
      {...interactionProps}
    />
  );
}
