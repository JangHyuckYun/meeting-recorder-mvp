import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { MinutesView } from "../components/MinutesView";
import { errorMessage, formatDate, formatDuration, STATUS_LABELS } from "../formatters";
import type { MinutesDraft, Recording, TranscriptSegment } from "../types";

interface RecordingDetail {
  recording: Recording;
  segments: TranscriptSegment[];
}

function EmptyHistory() {
  return (
    <div className="empty-state">
      <span className="empty-state-icon">◎</span>
      <h2>아직 저장된 회의가 없습니다</h2>
      <p>실시간 탭에서 첫 녹음을 시작하거나, 아래에서 기존 녹음 파일을 가져오세요.</p>
    </div>
  );
}

export function HistoryScreen() {
  const [recordings, setRecordings] = useState<Recording[]>([]);
  const [detail, setDetail] = useState<RecordingDetail | null>(null);
  const [minutesDraft, setMinutesDraft] = useState<MinutesDraft | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isDetailLoading, setIsDetailLoading] = useState(false);
  const [isTranscribing, setIsTranscribing] = useState(false);
  const [isGeneratingMinutes, setIsGeneratingMinutes] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // 기존 오디오 파일 가져오기(record_test_data 등 로컬 녹음 파일) — 실제 파일시스템 경로를
  // 직접 입력받는다. 리포지토리에는 어떤 오디오 파일도 커밋되지 않으므로 네이티브 파일
  // 다이얼로그 플러그인 없이도 동작하도록 절대 경로 입력 방식으로 최소 구현한다.
  const [importSourcePath, setImportSourcePath] = useState("");
  const [importTitle, setImportTitle] = useState("");
  const [isImporting, setIsImporting] = useState(false);
  const [importError, setImportError] = useState<string | null>(null);

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
      // 이 녹음에 대해 이전에 생성된 회의록이 있으면 재생성 없이 바로 복원한다.
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

  const importExistingFile = async () => {
    const sourcePath = importSourcePath.trim();
    if (!sourcePath) return;
    setIsImporting(true);
    setImportError(null);
    try {
      const title = importTitle.trim() || sourcePath.split("/").pop() || sourcePath;
      await invoke<Recording>("ingest_audio_file", { sourcePath, title });
      setImportSourcePath("");
      setImportTitle("");
      await loadRecordings();
    } catch (invokeError) {
      setImportError(errorMessage(invokeError));
    } finally {
      setIsImporting(false);
    }
  };

  if (detail) {
    const canTranscribe = ["recorded", "failed"].includes(detail.recording.status);
    const canGenerateMinutes = detail.segments.length > 0 && !minutesDraft;
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

      <form
        className="import-panel"
        onSubmit={(event) => {
          event.preventDefault();
          void importExistingFile();
        }}
      >
        <label htmlFor="import-source-path">기존 녹음 파일 가져오기 (절대 경로)</label>
        <div className="import-fields">
          <input
            id="import-source-path"
            value={importSourcePath}
            onChange={(event) => setImportSourcePath(event.currentTarget.value)}
            placeholder="/Users/.../회의녹음.m4a"
            disabled={isImporting}
          />
          <input
            value={importTitle}
            onChange={(event) => setImportTitle(event.currentTarget.value)}
            placeholder="회의 제목(선택)"
            disabled={isImporting}
          />
          <button type="submit" className="button primary" disabled={isImporting || !importSourcePath.trim()}>
            {isImporting ? "가져오는 중..." : "가져오기"}
          </button>
        </div>
        {importError && <div className="error-banner">가져오기 실패: {importError}</div>}
      </form>

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
