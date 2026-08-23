import { useCallback, useEffect } from "react";

import { hsl } from "@/lib/design-tokens";
import { cn } from "@/lib/utils";

import { GRID, WAVE, roundedRect } from "./geometry";
import { useCanvas, usePrefersReducedMotion, type CanvasSurface } from "./use-canvas";

/** One full sweep of the "awaiting signal" indicator. */
const SWEEP_PERIOD_MS = 1_800;
const SWEEP_WIDTH_RATIO = 0.18;

export interface WavVisualizerProps {
  /**
   * Normalized amplitudes in [0, 1], oldest first. The newest sample is drawn
   * at the right edge; older samples scroll out on the left.
   */
  levels?: readonly number[];
  /** True while capture is running. Idle surfaces render in the muted token. */
  active?: boolean;
  /** Surface height in CSS pixels. Defaults to the design-system wave height. */
  height?: number;
  /** Accessible name for the canvas. */
  label?: string;
  className?: string;
}

/**
 * Live input-level meter painted on canvas.
 *
 * It renders only the amplitudes it is given. With no samples it shows an
 * explicit "awaiting signal" baseline rather than inventing a waveform.
 */
export function WavVisualizer({
  levels = [],
  active = false,
  height = WAVE.height,
  label = "오디오 입력 레벨",
  className,
}: WavVisualizerProps) {
  const reducedMotion = usePrefersReducedMotion();
  const awaitingSignal = active && levels.length === 0;
  const sweeping = awaitingSignal && !reducedMotion;

  const paint = useCallback(
    ({ ctx, width, height: surfaceHeight, palette }: CanvasSurface) => {
      const midY = surfaceHeight / 2;
      const usableWidth = Math.max(0, width - WAVE.insetX * 2);
      if (usableWidth <= 0) return;

      ctx.fillStyle = hsl(palette.waveBaseline);
      ctx.fillRect(WAVE.insetX, midY - WAVE.baselineWidth / 2, usableWidth, WAVE.baselineWidth);

      if (levels.length === 0) {
        if (!awaitingSignal) return;
        const sweepWidth = usableWidth * SWEEP_WIDTH_RATIO;
        const phase = sweeping ? (performance.now() % SWEEP_PERIOD_MS) / SWEEP_PERIOD_MS : 0;
        const sweepX = WAVE.insetX + phase * (usableWidth + sweepWidth) - sweepWidth;
        ctx.save();
        ctx.beginPath();
        ctx.rect(WAVE.insetX, 0, usableWidth, surfaceHeight);
        ctx.clip();
        ctx.fillStyle = hsl(palette.waveActive, 0.55);
        roundedRect(
          ctx,
          sweepX,
          midY - WAVE.baselineWidth * 1.5,
          sweepWidth,
          WAVE.baselineWidth * 3,
          WAVE.cornerRadius,
        );
        ctx.fill();
        ctx.restore();
        return;
      }

      const slot = WAVE.barWidth + WAVE.barGap;
      const capacity = Math.max(1, Math.floor(usableWidth / slot));
      const visible = levels.slice(-capacity);
      const maxBarHeight = Math.max(WAVE.minBarHeight, surfaceHeight - GRID * 2);

      ctx.fillStyle = hsl(active ? palette.waveActive : palette.waveIdle);
      visible.forEach((level, index) => {
        const clamped = Math.min(1, Math.max(0, level));
        const barHeight = Math.max(WAVE.minBarHeight, clamped * maxBarHeight);
        const x = width - WAVE.insetX - (visible.length - index) * slot + WAVE.barGap;
        roundedRect(ctx, x, midY - barHeight / 2, WAVE.barWidth, barHeight, WAVE.cornerRadius);
        ctx.fill();
      });
    },
    [active, awaitingSignal, levels, sweeping],
  );

  const { canvasRef, repaint } = useCanvas(paint, { animate: sweeping });

  useEffect(() => {
    repaint();
  }, [levels, active, repaint]);

  return (
    <canvas
      ref={canvasRef}
      role="img"
      aria-label={label}
      className={cn("block w-full", className)}
      style={{ height }}
    />
  );
}
