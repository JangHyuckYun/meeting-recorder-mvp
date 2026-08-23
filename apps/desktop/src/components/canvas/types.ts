import type { CaptionState } from "@/lib/design-tokens";

export type { CaptionState };

/** One caption block on the timeline, in the recording's own time base. */
export interface CaptionSpan {
  id: string;
  startMs: number;
  endMs: number;
  /** Lifecycle state; drives color and fill treatment. */
  state: CaptionState;
  /** Speaker lane index, or null while diarization has not resolved a speaker. */
  speakerIndex?: number | null;
  /** Marks regions where more than one speaker was detected. */
  overlap?: boolean;
}

/** A contiguous stretch of speech attributed to one speaker. */
export interface SpeakerSegment {
  startMs: number;
  endMs: number;
  overlap?: boolean;
}

/** One diarized speaker lane. Labels are meeting-local, never identities. */
export interface SpeakerTrack {
  id: string;
  label: string;
  /** Index into the speaker color tokens; falls back to the neutral token. */
  colorIndex?: number | null;
  segments: readonly SpeakerSegment[];
}
