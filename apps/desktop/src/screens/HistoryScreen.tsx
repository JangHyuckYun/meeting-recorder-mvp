import { useCallback, useEffect, useState } from "react";
import { Button } from "@/components/ui";
import { appClient, type TranscriptionProgressEvent } from "@/platform/appClient";
import { MinutesView } from "../components/MinutesView";
import { errorMessage, formatDate, formatDuration, STATUS_LABELS } from "../formatters";
import type { MinutesDraft, Recording, TranscriptSegment } from "../types";

interface RecordingDetail {
  recording: Recording;
  segments: TranscriptSegment[];
}

function EmptyHistory() {
  return (
    <div className="history-empty">
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
      const result = await appClient.listRecordings();
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
      const [recording, segments] = await appClient.getRecordingDetail(id);
      setDetail({ recording, segments });
      const existingMinutes = await appClient.getMinutes(id);
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

    const unlisten = await appClient.onTranscriptionProgress((progress) => {
      if (progress.recording_id !== recId) return;
      setTranscriptionProgress((prev) => ({ ...prev, [recId]: progress }));
    });
    try {
      const segments = await appClient.transcribeRecording(recId);
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
      await appClient.cancelTranscription();
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
      await appClient.updateRecordingStatus(detail.recording.id, "recorded");
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
      const draft = await appClient.generateMinutes(detail.recording.id);
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
        <section className="transcript-panel history-detail-panel">
          <div className="history-detail-bar">
            <button type="button" className="history-back" onClick={() => setDetail(null)}>
              <span aria-hidden="true">←</span> 목록
            </button>
            <span className="history-status" data-status={detail.recording.status}>
              {STATUS_LABELS[detail.recording.status]}
            </span>
          </div>

          <header className="history-detail-head">
            <h1>{detail.recording.title}</h1>
            <p className="history-detail-meta">
              <span data-numeric>{formatDate(detail.recording.created_at)}</span>
              <span aria-hidden="true">·</span>
              <span data-numeric>{formatDuration(detail.recording.duration_ms)}</span>
            </p>
          </header>

          {error && (
            <div className="error-banner" role="alert">
              요청을 처리하지 못했습니다: {error}
            </div>
          )}

          <div className="history-toolbar">
            <h2>전사 내용</h2>
            <span className="history-toolbar-count" data-numeric>
              {detail.segments.length}개 세그먼트
            </span>
            {canTranscribe && !isThisTranscribing && (
              <Button size="sm" onClick={transcribeRecording}>
                전사 시작
              </Button>
            )}
            {isThisTranscribing && (
              <Button
                variant="outline"
                size="sm"
                className="text-destructive border-destructive/25 hover:bg-destructive-soft hover:text-destructive"
                onClick={cancelTranscription}
              >
                전사 취소
              </Button>
            )}
            {canRetry && !isThisTranscribing && (
              <Button size="sm" onClick={retryTranscription}>
                재시도
              </Button>
            )}
            {detail.recording.status === "transcribing" && !isThisTranscribing && (
              <Button
                variant="outline"
                size="sm"
                className="text-warning border-warning/25 hover:bg-warning-soft hover:text-warning"
                onClick={retryTranscription}
              >
                상태 초기화
              </Button>
            )}
          </div>

          {isThisTranscribing && (
            <div className="history-progress" aria-live="polite">
              <progress
                className="history-progress-bar"
                aria-label="전사 진행률"
                max={100}
                value={thisProgress ? progressPercent : undefined}
              />
              <span className="history-progress-text" data-numeric>
                {progressText ?? "전사 준비 중..."} {thisProgress ? `${Math.round(progressPercent)}%` : ""}
              </span>
            </div>
          )}

          <div className="history-transcript ds-scroll">
            {detail.segments.length > 0 ? (
              detail.segments.map((segment, index) => (
                <article className="history-segment" key={segment.id}>
                  <time className="history-segment-time" data-numeric>
                    {formatDuration(segment.start_ms)}
                  </time>
                  <div>
                    <div className="history-segment-meta">
                      <span className="history-speaker" data-speaker={index % 4}>
                        {segment.speaker_label}
                      </span>
                      {!segment.is_final && <span className="history-segment-flag">처리 중</span>}
                    </div>
                    <p>{segment.text}</p>
                  </div>
                </article>
              ))
            ) : (
              <div className="history-note">
                <h3>아직 전사 내용이 없습니다</h3>
                <p>전사 시작 버튼을 눌러 화자별 회의 내용을 생성하세요.</p>
              </div>
            )}
          </div>
        </section>

        {minutesDraft ? (
          <MinutesView minutes={minutesDraft} recordingId={detail.recording.id} onItemEdited={handleItemEdited} />
        ) : (
          <section className="minutes-view history-minutes" aria-label="회의록 초안">
            <header className="history-minutes-header">
              <h2>회의록 초안</h2>
              <span className="history-minutes-eyebrow">AI MEETING NOTES</span>
            </header>
            <div className="history-note">
              {canGenerateMinutes ? (
                <>
                  <h3>아직 회의록이 없습니다</h3>
                  <p>전사 내용을 바탕으로 결정 사항과 할 일을 생성하세요.</p>
                  <Button
                    size="sm"
                    className="history-note-action"
                    disabled={isGeneratingMinutes}
                    onClick={generateMinutes}
                  >
                    {isGeneratingMinutes ? "회의록 생성 중..." : "회의록 생성"}
                  </Button>
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
      <header className="history-topbar">
        <h1>회의 히스토리</h1>
        <span className="history-count" data-numeric>
          {recordings.length}건
        </span>
        <Button variant="outline" size="sm" onClick={loadRecordings}>
          새로고침
        </Button>
      </header>

      <div className="history-list ds-scroll">
        {error && (
          <div className="error-banner" role="alert">
            목록을 불러오지 못했습니다: {error}
          </div>
        )}

        {isLoading ? (
          <p className="history-loading">
            <span aria-hidden="true" />
            녹음 목록을 불러오는 중...
          </p>
        ) : recordings.length === 0 ? (
          error ? null : <EmptyHistory />
        ) : (
          <>
            <div className="history-row history-row-head" aria-hidden="true">
              <span>제목</span>
              <span>상태</span>
              <span>기록 시각</span>
              <span className="history-cell-duration">길이</span>
            </div>
            {recordings.map((recording) => (
              <button
                type="button"
                className="history-row"
                key={recording.id}
                onClick={() => openRecording(recording.id)}
                disabled={isDetailLoading}
              >
                <span className="history-cell-title">{recording.title}</span>
                <span className="history-status" data-status={recording.status}>
                  {STATUS_LABELS[recording.status]}
                </span>
                <span className="history-cell-date" data-numeric>
                  {formatDate(recording.created_at)}
                </span>
                <span className="history-cell-duration" data-numeric>
                  {formatDuration(recording.duration_ms)}
                </span>
              </button>
            ))}
          </>
        )}
      </div>
    </section>
  );
}
