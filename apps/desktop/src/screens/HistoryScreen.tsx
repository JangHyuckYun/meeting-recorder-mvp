import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { MinutesView } from "../components/MinutesView";
import { errorMessage, formatDate, formatDuration, STATUS_LABELS } from "../formatters";
import type { Recording, TranscriptSegment } from "../types";

interface RecordingDetail {
  recording: Recording;
  segments: TranscriptSegment[];
}

function EmptyHistory() {
  return (
    <div className="empty-state">
      <span className="empty-state-icon">◎</span>
      <h2>아직 저장된 회의가 없습니다</h2>
      <p>실시간 탭에서 첫 녹음을 시작해보세요.</p>
    </div>
  );
}

export function HistoryScreen() {
  const [recordings, setRecordings] = useState<Recording[]>([]);
  const [detail, setDetail] = useState<RecordingDetail | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isDetailLoading, setIsDetailLoading] = useState(false);
  const [isTranscribing, setIsTranscribing] = useState(false);
  const [error, setError] = useState<string | null>(null);

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
    try {
      const [recording, segments] = await invoke<[Recording, TranscriptSegment[]]>(
        "get_recording_detail",
        { id },
      );
      setDetail({ recording, segments });
    } catch (invokeError) {
      setError(errorMessage(invokeError));
    } finally {
      setIsDetailLoading(false);
    }
  };

  const transcribeRecording = async () => {
    if (!detail) return;
    setIsTranscribing(true);
    setError(null);
    try {
      const segments = await invoke<TranscriptSegment[]>("transcribe_recording", {
        id: detail.recording.id,
      });
      const recording = { ...detail.recording, status: "transcribed" as const };
      setDetail({ recording, segments });
      setRecordings((current) =>
        current.map((item) => (item.id === recording.id ? recording : item)),
      );
    } catch (invokeError) {
      setError(errorMessage(invokeError));
    } finally {
      setIsTranscribing(false);
    }
  };

  if (detail) {
    const canTranscribe = ["recorded", "failed"].includes(detail.recording.status);
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
            {canTranscribe && (
              <button
                type="button"
                className="button primary"
                disabled={isTranscribing}
                onClick={transcribeRecording}
              >
                {isTranscribing ? "전사 생성 중..." : "전사 시작"}
              </button>
            )}
          </div>

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
        <MinutesView />
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
