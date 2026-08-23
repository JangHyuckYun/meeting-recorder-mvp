import { useCallback, useEffect } from "react";

import { hsl, speakerColor } from "@/lib/design-tokens";
import { cn } from "@/lib/utils";

import { GRID, TRACKS, roundedRect } from "./geometry";
import type { SpeakerTrack } from "./types";
import { useCanvas, type CanvasSurface } from "./use-canvas";

const MIN_SEGMENT_WIDTH = GRID * 0.75;

export interface SpeakerTracksProps {
  speakers?: readonly SpeakerTrack[];
  /** Total recording length in ms; drives the horizontal scale. */
  durationMs: number;
  cursorMs?: number | null;
  label?: string;
  emptyLabel?: string;
  className?: string;
}

/** Height the canvas needs for the given number of lanes. */
function surfaceHeightFor(laneCount: number): number {
  const lanes = Math.max(1, laneCount);
  return TRACKS.paddingY * 2 + lanes * TRACKS.laneHeight + (lanes - 1) * TRACKS.laneGap;
}

/**
 * One lane per diarized speaker with their speech segments on a shared time axis.
 * Labels are meeting-local clusters, never verified identities.
 */
export function SpeakerTracks({
  speakers = [],
  durationMs,
  cursorMs = null,
  label = "화자 트랙",
  emptyLabel = "화자 분리 결과 없음",
  className,
}: SpeakerTracksProps) {
  const paint = useCallback(
    ({ ctx, width, height, palette }: CanvasSurface) => {
      const trackLeft = TRACKS.gutterWidth;
      const usableWidth = Math.max(0, width - trackLeft - TRACKS.insetX);
      if (usableWidth <= 0) return;

      const scale = durationMs > 0 ? usableWidth / durationMs : 0;
      const toX = (ms: number) => trackLeft + Math.min(usableWidth, Math.max(0, ms * scale));

      if (speakers.length === 0) {
        ctx.fillStyle = hsl(palette.subtleForeground);
        ctx.font = `${TRACKS.fontSize}px ${palette.fontSans}`;
        ctx.textAlign = "center";
        ctx.textBaseline = "middle";
        ctx.fillText(emptyLabel, width / 2, height / 2);
        return;
      }

      speakers.forEach((speaker, index) => {
        const laneTop = TRACKS.paddingY + index * (TRACKS.laneHeight + TRACKS.laneGap);
        const laneColor = speakerColor(palette, speaker.colorIndex ?? index);

        // --- gutter: color chip + label -------------------------------
        ctx.fillStyle = hsl(laneColor);
        roundedRect(
          ctx,
          TRACKS.insetX,
          laneTop + TRACKS.laneHeight / 2 - GRID,
          GRID * 2,
          GRID * 2,
          GRID * 0.5,
        );
        ctx.fill();

        ctx.fillStyle = hsl(palette.mutedForeground);
        ctx.font = `600 ${TRACKS.fontSize}px ${palette.fontSans}`;
        ctx.textAlign = "left";
        ctx.textBaseline = "middle";
        const labelX = TRACKS.insetX + GRID * 3;
        const labelWidth = trackLeft - labelX - GRID;
        let text = speaker.label;
        while (text.length > 1 && ctx.measureText(text).width > labelWidth) {
          text = text.slice(0, -1);
        }
        ctx.fillText(
          text === speaker.label ? text : `${text}…`,
          labelX,
          laneTop + TRACKS.laneHeight / 2,
        );

        // --- lane background ------------------------------------------
        ctx.fillStyle = hsl(palette.waveBaseline, 0.6);
        roundedRect(ctx, trackLeft, laneTop, usableWidth, TRACKS.laneHeight, TRACKS.laneRadius);
        ctx.fill();

        // --- segments ---------------------------------------------------
        for (const segment of speaker.segments) {
          const startX = toX(segment.startMs);
          const endX = toX(Math.max(segment.startMs, segment.endMs));
          const segmentWidth = Math.max(MIN_SEGMENT_WIDTH, endX - startX);
          ctx.fillStyle = hsl(laneColor, 0.85);
          roundedRect(
            ctx,
            startX,
            laneTop + TRACKS.segmentInset,
            segmentWidth,
            TRACKS.laneHeight - TRACKS.segmentInset * 2,
            TRACKS.laneRadius,
          );
          ctx.fill();

          if (segment.overlap) {
            ctx.fillStyle = hsl(palette.waveOverlap);
            ctx.fillRect(startX, laneTop + TRACKS.segmentInset, segmentWidth, TRACKS.overlapStripe / 2);
          }
        }
      });

      // --- playhead across every lane -----------------------------------
      if (cursorMs !== null && cursorMs !== undefined && durationMs > 0) {
        const x = toX(cursorMs);
        ctx.fillStyle = hsl(palette.waveCursor);
        ctx.fillRect(x - 1, TRACKS.paddingY / 2, 2, height - TRACKS.paddingY);
      }
    },
    [cursorMs, durationMs, emptyLabel, speakers],
  );

  const { canvasRef, repaint } = useCanvas(paint);

  useEffect(() => {
    repaint();
  }, [cursorMs, durationMs, repaint, speakers]);

  return (
    <canvas
      ref={canvasRef}
      role="img"
      aria-label={label}
      className={cn("block w-full", className)}
      style={{ height: surfaceHeightFor(speakers.length) }}
    />
  );
}
