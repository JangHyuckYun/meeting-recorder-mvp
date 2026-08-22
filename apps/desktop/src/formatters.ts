import type { RecordingStatus } from "./types";

export function formatDuration(milliseconds: number | null): string {
  if (milliseconds === null) return "--:--";
  const totalSeconds = Math.max(0, Math.floor(milliseconds / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  return hours > 0
    ? [hours, minutes, seconds].map((value) => String(value).padStart(2, "0")).join(":")
    : [minutes, seconds].map((value) => String(value).padStart(2, "0")).join(":");
}

export function formatDate(date: string): string {
  return new Intl.DateTimeFormat("ko-KR", {
    month: "long",
    day: "numeric",
    weekday: "short",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(date));
}

export const STATUS_LABELS: Record<RecordingStatus, string> = {
  recording: "녹음 중",
  recorded: "전사 대기",
  transcribing: "전사 중",
  transcribed: "전사 완료",
  minutes_ready: "회의록 완료",
  failed: "처리 실패",
};

export function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
