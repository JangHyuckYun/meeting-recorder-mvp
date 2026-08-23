import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { MinutesView } from "../components/MinutesView";
import { errorMessage, formatDate, formatDuration, STATUS_LABELS } from "../formatters";
import type { MinutesDraft, Recording, TranscriptSegment } from "../types";

interface RecordingDetail {
  recording: Recording;
  segments: TranscriptSegment[];
}

type TranscriptionProgressEvent = {
  recording_id: string;
  sent_ms: number;
  total_ms: number;
  phase: "sending" | "finalizing" | "done";
};

function EmptyHistory() {
  return (
    <div className="empty-state">
      <span className="empty-state-icon">◎</span>
      <h2>아직 저장된 회의가 없습니다</h2>
      <p>실시간 탭에서 첫 녹음을 시작하거나, 가져오기 탭에서 기존 녹음 파일을 가져오세요.</p>
    </div>
  );
}

export function HistoryScreen() {
  const [recordings, setRecordings] = useState<Recording[]>([]);
  const [detail, setDetail] = useState<RecordingDetail | null>(null);
  const [minutesDraft, setMinutesDraft] = useState<MinutesDraft | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isDetailLoading, setIsDetailLoading] = useState(false);
  const [isGeneratingMinutes, setIsGeneratingMinutes] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [transcribingRecordings, setTranscribingRecordings] = useState<Set<string>>(new Set());
  const [transcriptionProgress, setTranscriptionProgress] = useState<Record<string, TranscriptionProgressEvent>>({});

  const loadRecordings = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const result = await invoke<Recording[]>("list_recordings");
      setRecordings(result);
    } catch (invokeError) {
      setError(errorMessage(invokeError));
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadRecordings();
  }, [loadRecordings]);

  const openRecording = async (id: string) => {
    setIsDetailLoading(true);
    setError(null);
    setMinutesDraft(null);
    try {
      const [recording, segments] = await invoke<[Recording, TranscriptSegment[]]>(
        "get_recording_detail",
        { id },
      );
      setDetail({ recording, segments });
      const existingMinutes = await invoke<MinutesDraft | null>("get_minutes", {
        recordingId: id,
      });
      if (existingMinutes) setMinutesDraft(existingMinutes);
    } catch (invokeError) {
      setError(errorMessage(invokeError));
    } finally {
      setIsDetailLoading(false);
    }
  };

  const transcribeRecording = async () => {
    if (!detail) return;
    const recId = detail.recording.id;
    setTranscribingRecordings((prev) => new Set(prev).add(recId));
    setTranscriptionProgress((prev) => ({ ...prev, [recId]: null as unknown as TranscriptionProgressEvent }));
    setError(null);

    const { listen } = await import("@tauri-apps/api/event");
    const unlisten = await listen<TranscriptionProgressEvent>(
      "transcription-progress",
      (event) => {
        if (event.payload.recording_id !== recId) return;
        setTranscriptionProgress((prev) => ({ ...prev, [recId]: event.payload }));
      },
    );
    try {
      const segments = await invoke<TranscriptSegment[]>("transcribe_recording", {
        id: recId,
      });
      const recording = { ...detail.recording, status: "transcribed" as const };
      setDetail({ recording, segments });
      setRecordings((current) =>
        current.map((item) => (item.id === recording.id ? recording : item)),
      );
    } catch (invokeError) {
      setError(errorMessage(invokeError));
    } finally {
      setTranscribingRecordings((prev) => {
        const next = new Set(prev);
        next.delete(recId);
        return next;
      });
      unlisten();
    }
  };

  const cancelTranscription = async () => {
    try {
      await invoke("cancel_transcription");
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const retryTranscription = async () => {
    if (!detail) return;
    // Reset status to recorded so the transcribe button reappears
    // Also update the backend DB
    try {
      await invoke("update_recording_status", { id: detail.recording.id, status: "recorded" });
    } catch { /* ignore */ }
    const updated = { ...detail.recording, status: "recorded" as const };
    setDetail({
      ...detail,
      recording: updated,
    });
    setRecordings((current) =>
      current.map((item) => (item.id === updated.id ? updated : item)),
    );
    setError(null);
  };

  const generateMinutes = async () => {
    if (!detail) return;
    setIsGeneratingMinutes(true);
    setError(null);
    try {
      const draft = await invoke<MinutesDraft>("generate_minutes", {
        recordingId: detail.recording.id,
      });
      setMinutesDraft(draft);
    } catch (invokeError) {
      setError(errorMessage(invokeError));
    } finally {
      setIsGeneratingMinutes(false);
    }
  };

  const handleItemEdited = (
    section: "decisions" | "action_items",
    itemId: string,
    text: string,
  ) => {
    setMinutesDraft((current) => {
      if (!current) return current;
      return {
        ...current,
        [section]: current[section].map((item) =>
          item.id === itemId ? { ...item, text } : item,
        ),
      };
    });
  };

  if (detail) {
    const recId = detail.recording.id;
    const isThisTranscribing = transcribingRecordings.has(recId);
    const canTranscribe = detail.recording.status === "recorded";
    const canRetry = detail.recording.status === "failed";
    const canGenerateMinutes = detail.segments.length > 0 && !minutesDraft;
    const thisProgress = transcriptionProgress[recId];
    const progressPercent =
      thisProgress && thisProgress.total_ms > 0
        ? Math.min(100, Math.max(0, (thisProgress.sent_ms / thisProgress.total_ms) * 100))
        : 0;
    const progressText = (() => {
      if (!thisProgress) return null;
      if (thisProgress.phase === "finalizing") return "완료 처리 중...";
      if (thisProgress.phase === "done") return "완료";
      const remainingMs = thisProgress.total_ms - thisProgress.sent_ms;
      const remainingSec = Math.max(0, Math.round(remainingMs / 1000));
      return `약 ${remainingSec}초 남음`;
    })();
    return (
      <div className="history-detail-screen">
        <section className="transcript-panel detail-panel">
          <header className="panel-header detail-header">
            <button type="button" className="back-button" onClick={() => setDetail(null)}>
              <span aria-hidden="true">←</span> 목록
            </button>
            <div className="detail-heading-copy">
              <p className="eyebrow">RECORDING DETAIL</p>
              <h1>{detail.recording.title}</h1>
              <p className="recording-meta">
                {formatDate(detail.recording.created_at)} · {formatDuration(detail.recording.duration_ms)}
              </p>
            </div>
            <span className={`recording-status ${detail.recording.status}`}>
              {STATUS_LABELS[detail.recording.status]}
            </span>
          </header>

          {error && <div className="error-banner">요청을 처리하지 못했습니다: {error}</div>}

          <div className="transcript-toolbar">
            <div>
              <h2>전사 내용</h2>
              <span>{detail.segments.length}개 세그먼트</span>
            </div>
            {canTranscribe && !isThisTranscribing && (
              <button
                type="button"
                className="button primary"
                onClick={transcribeRecording}
              >
                전사 시작
              </button>
            )}
            {isThisTranscribing && (
              <button
                type="button"
                className="button secondary"
                style={{ color: "hsl(var(--destructive))", borderColor: "hsl(var(--destructive) / 0.25)" }}
                onClick={cancelTranscription}
              >
                전사 취소
              </button>
            )}
            {canRetry && !isThisTranscribing && (
              <button
                type="button"
                className="button primary"
                onClick={retryTranscription}
              >
                재시도
              </button>
            )}
            {detail.recording.status === "transcribing" && !isThisTranscribing && (
              <button
                type="button"
                className="button secondary"
                style={{ color: "hsl(var(--warning))", borderColor: "hsl(var(--warning) / 0.25)" }}
                onClick={retryTranscription}
              >
                상태 초기화
              </button>
            )}
          </div>

          {isThisTranscribing && (
            <div className="transcription-progress" aria-live="polite">
              <div className="progress-track">
                <div className="progress-fill" style={{ width: `${progressPercent}%` }} />
              </div>
              <span className="progress-text">
                {progressText ?? "전사 준비 중..."} {thisProgress ? `${Math.round(progressPercent)}%` : ""}
              </span>
            </div>
          )}

          <div className="transcript-list">
            {detail.segments.length > 0 ? (
              detail.segments.map((segment, index) => (
                <article className="transcript-segment" key={segment.id}>
                  <div className={`speaker-avatar color-${index % 4}`}>
                    {segment.speaker_label.slice(-1)}
                  </div>
                  <div className="segment-content">
                    <div className="segment-meta">
                      <strong>{segment.speaker_label}</strong>
                      <time>{formatDuration(segment.start_ms)}</time>
                      {!segment.is_final && <span>처리 중</span>}
                    </div>
                    <p>{segment.text}</p>
                  </div>
                </article>
              ))
            ) : (
              <div className="empty-transcript">
                <h3>아직 전사 내용이 없습니다</h3>
                <p>전사 시작 버튼을 눌러 화자별 회의 내용을 생성하세요.</p>
              </div>
            )}
          </div>
        </section>

        {minutesDraft ? (
          <MinutesView minutes={minutesDraft} recordingId={detail.recording.id} onItemEdited={handleItemEdited} />
        ) : (
          <section className="minutes-view" aria-label="회의록 초안">
            <header className="panel-header">
              <div>
                <p className="eyebrow">AI MEETING NOTES</p>
                <h2>회의록 초안</h2>
              </div>
            </header>
            <div className="empty-transcript">
              {canGenerateMinutes ? (
                <>
                  <h3>아직 회의록이 없습니다</h3>
                  <p>전사 내용을 바탕으로 결정 사항과 할 일을 생성하세요.</p>
                  <button
                    type="button"
                    className="button primary"
                    disabled={isGeneratingMinutes}
                    onClick={generateMinutes}
                  >
                    {isGeneratingMinutes ? "회의록 생성 중..." : "회의록 생성"}
                  </button>
                </>
              ) : (
                <>
                  <h3>전사가 먼저 필요합니다</h3>
                  <p>전사 시작 버튼으로 화자별 내용을 먼저 생성한 뒤 회의록을 만들 수 있습니다.</p>
                </>
              )}
            </div>
          </section>
        )}
      </div>
    );
  }

  return (
    <section className="history-screen">
      <header className="history-header">
        <div>
          <p className="eyebrow">ARCHIVE</p>
          <h1>회의 히스토리</h1>
          <p>저장된 녹음과 전사 내용을 한곳에서 확인하세요.</p>
        </div>
        <button type="button" className="button secondary refresh-button" onClick={loadRecordings}>
          새로고침
        </button>
      </header>

      {error && <div className="error-banner">목록을 불러오지 못했습니다: {error}</div>}

      {isLoading ? (
        <div className="loading-state"><span />녹음 목록을 불러오는 중...</div>
      ) : recordings.length === 0 ? (
        <EmptyHistory />
      ) : (
        <div className="recording-grid">
          {recordings.map((recording) => (
            <button
              type="button"
              className="recording-card"
              key={recording.id}
              onClick={() => openRecording(recording.id)}
              disabled={isDetailLoading}
            >
              <div className="card-topline">
                <span className={`recording-status ${recording.status}`}>
                  {STATUS_LABELS[recording.status]}
                </span>
                <span className="card-arrow">↗</span>
              </div>
              <div>
                <h2>{recording.title}</h2>
                <p>{formatDate(recording.created_at)}</p>
              </div>
              <div className="card-duration">
                <span className="mini-wave" aria-hidden="true"><i /><i /><i /><i /><i /></span>
                {formatDuration(recording.duration_ms)}
              </div>
            </button>
          ))}
        </div>
      )}
    </section>
  );
}
