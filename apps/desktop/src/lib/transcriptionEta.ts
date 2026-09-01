export interface ProgressSample {
  at: number;
  fraction: number;
}

export function estimateRemainingMs(samples: ProgressSample[], now: number): number | null {
  const last = samples[samples.length - 1];
  if (!last || last.fraction >= 1) return last ? 0 : null;
  if (last.fraction < 0.05 || now - samples[0].at < 10_000) return null;
  const first = samples[0];
  const elapsed = last.at - first.at;
  const gained = last.fraction - first.fraction;
  if (elapsed <= 0 || gained <= 0) return null;
  return Math.max(0, Math.round((now - last.at + (1 - last.fraction) * (elapsed / gained))));
}

export function formatRemainingMs(ms: number | null): string {
  if (ms === null) return "계산 중…";
  const seconds = Math.max(0, Math.ceil(ms / 1000));
  if (seconds < 60) return `약 ${seconds}초 남음`;
  return `약 ${Math.floor(seconds / 60)}분 ${seconds % 60}초 남음`;
}
