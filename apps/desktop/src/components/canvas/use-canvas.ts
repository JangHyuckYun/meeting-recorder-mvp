import { useCallback, useEffect, useRef, useState } from "react";

import { readCanvasPalette, type CanvasPalette } from "@/lib/design-tokens";

export interface CanvasSurface {
  readonly ctx: CanvasRenderingContext2D;
  /** CSS pixel width of the surface (device pixel ratio already applied to ctx). */
  readonly width: number;
  /** CSS pixel height of the surface. */
  readonly height: number;
  readonly palette: CanvasPalette;
}

export type CanvasPainter = (surface: CanvasSurface) => void;

/** True when the user asked the OS to reduce motion. Updates live. */
export function usePrefersReducedMotion(): boolean {
  const [reduced, setReduced] = useState(() =>
    typeof window === "undefined"
      ? false
      : window.matchMedia("(prefers-reduced-motion: reduce)").matches,
  );

  useEffect(() => {
    const query = window.matchMedia("(prefers-reduced-motion: reduce)");
    const onChange = (event: MediaQueryListEvent) => setReduced(event.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);

  return reduced;
}

interface UseCanvasOptions {
  /** Repaint on every animation frame instead of only on size/paint changes. */
  animate?: boolean;
}

/**
 * Owns the boilerplate every canvas component needs: device-pixel-ratio scaling,
 * resize observation, design-token resolution, and repaint scheduling.
 *
 * The painter receives CSS-pixel dimensions; the context is pre-scaled so no
 * component has to reason about DPR.
 */
export function useCanvas(paint: CanvasPainter, options: UseCanvasOptions = {}) {
  const { animate = false } = options;
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const paintRef = useRef(paint);
  const paletteRef = useRef<CanvasPalette | null>(null);
  const sizeRef = useRef({ width: 0, height: 0 });
  const frameRef = useRef<number | null>(null);

  paintRef.current = paint;

  const render = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const { width, height } = sizeRef.current;
    if (width <= 0 || height <= 0) return;

    const palette = (paletteRef.current ??= readCanvasPalette(canvas));
    const ratio = window.devicePixelRatio || 1;
    const pixelWidth = Math.max(1, Math.round(width * ratio));
    const pixelHeight = Math.max(1, Math.round(height * ratio));

    if (canvas.width !== pixelWidth || canvas.height !== pixelHeight) {
      canvas.width = pixelWidth;
      canvas.height = pixelHeight;
    }

    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    ctx.clearRect(0, 0, width, height);
    paintRef.current({ ctx, width, height, palette });
  }, []);

  const schedule = useCallback(() => {
    if (frameRef.current !== null) return;
    frameRef.current = window.requestAnimationFrame(() => {
      frameRef.current = null;
      render();
    });
  }, [render]);

  // Track the element box in CSS pixels; repaint whenever it changes.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    paletteRef.current = readCanvasPalette(canvas);
    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      const box = entry.contentRect;
      sizeRef.current = { width: box.width, height: box.height };
      schedule();
    });
    observer.observe(canvas);

    const rect = canvas.getBoundingClientRect();
    sizeRef.current = { width: rect.width, height: rect.height };
    schedule();

    return () => observer.disconnect();
  }, [schedule]);

  // Continuous mode is opt-in and used only while something is genuinely live.
  useEffect(() => {
    if (!animate) return;
    let running = true;
    const loop = () => {
      if (!running) return;
      render();
      frameRef.current = window.requestAnimationFrame(loop);
    };
    frameRef.current = window.requestAnimationFrame(loop);
    return () => {
      running = false;
      if (frameRef.current !== null) {
        window.cancelAnimationFrame(frameRef.current);
        frameRef.current = null;
      }
    };
  }, [animate, render]);

  return { canvasRef, repaint: schedule };
}
