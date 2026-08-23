/**
 * Canvas painting cannot use Tailwind classes, so it reads the same CSS custom
 * properties the utility classes compile to. Nothing in components/canvas is
 * allowed to hardcode a color — it resolves tokens through this module.
 */

/** Number of distinct speaker lane colors defined in global.css. */
export const SPEAKER_COLOR_COUNT = 6;

/** Caption lifecycle states shared with the ASR event contract. */
export type CaptionState = "partial" | "stable" | "committed" | "revised";

export interface CanvasPalette {
  readonly surface: string;
  readonly border: string;
  readonly borderStrong: string;
  readonly foreground: string;
  readonly mutedForeground: string;
  readonly subtleForeground: string;
  readonly waveActive: string;
  readonly waveIdle: string;
  readonly waveBaseline: string;
  readonly waveGrid: string;
  readonly waveCursor: string;
  readonly wavePeak: string;
  readonly waveOverlap: string;
  readonly state: Readonly<Record<CaptionState, string>>;
  readonly speakers: readonly string[];
  readonly speakerUnknown: string;
  readonly fontSans: string;
  readonly fontMono: string;
}

/**
 * Reads a raw token value (an HSL triplet such as `221.2 83.2% 53.3%`).
 * Custom properties defined via `var()` chains resolve to their final value.
 */
function readToken(styles: CSSStyleDeclaration, name: string): string {
  return styles.getPropertyValue(name).trim();
}

/** Wraps an HSL triplet into a canvas-usable color, optionally with alpha. */
export function hsl(triplet: string, alpha = 1): string {
  return alpha >= 1 ? `hsl(${triplet})` : `hsl(${triplet} / ${alpha})`;
}

/** Resolves every token the canvas layer paints with, in one computed-style read. */
export function readCanvasPalette(element: Element): CanvasPalette {
  const styles = getComputedStyle(element);
  const speakers: string[] = [];
  for (let index = 1; index <= SPEAKER_COLOR_COUNT; index += 1) {
    speakers.push(readToken(styles, `--speaker-${index}`));
  }

  return {
    surface: readToken(styles, "--surface"),
    border: readToken(styles, "--border"),
    borderStrong: readToken(styles, "--border-strong"),
    foreground: readToken(styles, "--foreground"),
    mutedForeground: readToken(styles, "--muted-foreground"),
    subtleForeground: readToken(styles, "--subtle-foreground"),
    waveActive: readToken(styles, "--wave-active"),
    waveIdle: readToken(styles, "--wave-idle"),
    waveBaseline: readToken(styles, "--wave-baseline"),
    waveGrid: readToken(styles, "--wave-grid"),
    waveCursor: readToken(styles, "--wave-cursor"),
    wavePeak: readToken(styles, "--wave-peak"),
    waveOverlap: readToken(styles, "--wave-overlap"),
    state: {
      partial: readToken(styles, "--state-partial"),
      stable: readToken(styles, "--state-stable"),
      committed: readToken(styles, "--state-committed"),
      revised: readToken(styles, "--state-revised"),
    },
    speakers,
    speakerUnknown: readToken(styles, "--speaker-unknown"),
    fontSans: readToken(styles, "--font-sans"),
    fontMono: readToken(styles, "--font-mono"),
  };
}

/** Stable lane color for a speaker index; unknown speakers get the neutral token. */
export function speakerColor(palette: CanvasPalette, index: number | null | undefined): string {
  if (index === null || index === undefined || index < 0) return palette.speakerUnknown;
  return palette.speakers[index % palette.speakers.length] ?? palette.speakerUnknown;
}
