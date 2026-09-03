import { useSyncExternalStore } from "react";
import { appClient, type TranscriptionProgressEvent } from "@/platform/appClient";
import type { Recording } from "@/types";

export interface ImportJob {
  recording: Recording;
  progress: TranscriptionProgressEvent;
  startedAt: number;
  samples: { at: number; fraction: number }[];
  log: string[];
}

let job: ImportJob | null = null;
const listeners = new Set<() => void>();
let listening = false;

const emit = () => listeners.forEach((listener) => listener());
const update = (progress: TranscriptionProgressEvent, recording = job?.recording) => {
  if (!recording) return;
  const at = Date.now();
  const fraction = progress.total_ms ? Math.min(1, progress.sent_ms / progress.total_ms) : 0;
  job = {
    recording,
    progress,
    startedAt: job?.startedAt ?? at,
    samples: [...(job?.samples ?? []), { at, fraction }].slice(-6),
    log: [...(job?.log ?? []), `${progress.phase}: ${Math.round(fraction * 100)}%`].slice(-20),
  };
  emit();
};

function ensureListening() {
  if (listening) return;
  listening = true;
  void appClient.onTranscriptionProgress(update);
  void importStore.hydrate().catch(() => {});
}

export const importStore = {
  getSnapshot: () => job,
  subscribe: (listener: () => void) => {
    ensureListening();
    listeners.add(listener);
    return () => listeners.delete(listener);
  },
  start: (recording: Recording) => {
    const now = Date.now();
    job = {
      recording,
      progress: { recording_id: recording.id, sent_ms: 0, total_ms: recording.duration_ms ?? 0, phase: "sending" },
      startedAt: now,
      samples: [{ at: now, fraction: 0 }],
      log: ["전사 시작"],
    };
    ensureListening();
    emit();
  },
  hydrate: async () => {
    ensureListening();
    const active = await appClient.getActiveTranscriptions();
    if (!active.length) return;
    const recordings = await appClient.listRecordings();
    const recording = recordings.find(({ id }) => id === active[0].recording_id);
    if (recording) update(active[0], recording);
  },
  reset: () => {
    job = null;
    emit();
  },
};

export function useImportJob() {
  return useSyncExternalStore(importStore.subscribe, importStore.getSnapshot, importStore.getSnapshot);
}
