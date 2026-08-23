//! React hook that subscribes to the Tauri `caption-update` event and maintains
//! a mutable Map of CaptionEvent segments for the Canvas components to consume.

import { useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface CaptionEventData {
  segment_id: string;
  start_sample: number;
  end_sample: number;
  text: string;
  status: "absent" | "partial" | "stable" | "committed" | "revised";
  speaker_label: string | null;
  overlap?: { speaker_count: number; retired_label: string | null };
  supersedes?: string[];
}

export interface CaptionState {
  /** Current segment map, keyed by segment_id. */
  segments: Map<string, CaptionEventData>;
  /** Sequence number for the last applied event. */
  sequence: number;
}

/**
 * Subscribe to the Tauri `caption-update` event. Returns a live CaptionState
 * that the Canvas components can render via CaptionTimeline or WavVisualizer.
 */
export function useCaptionEvents(): CaptionState {
  const [state, setState] = useState<CaptionState>({
    segments: new Map(),
    sequence: 0,
  });
  const segmentsRef = useRef(new Map<string, CaptionEventData>());

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;

    const setup = async () => {
      unlisten = await listen<CaptionEventData>("caption-update", (event) => {
        const ev = event.payload;
        const map = segmentsRef.current;

        // Handle supersession: remove superseded segments
        if (ev.supersedes && ev.supersedes.length > 0) {
          for (const supersededId of ev.supersedes) {
            map.delete(supersededId);
          }
        }

        // Apply the new event
        map.set(ev.segment_id, ev);

        // Trigger a React re-render with a new map reference
        setState({
          segments: new Map(map),
          sequence: Date.now(),
        });
      });
    };

    setup();

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  return state;
}