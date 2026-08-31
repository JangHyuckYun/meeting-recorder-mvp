import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { CaptionTimeline, SpeakerTracks, WavVisualizer } from "@/components/canvas";
import { Badge, Button } from "@/components/ui";
import { cn } from "@/lib/utils";
import { MinutesView } from "../components/MinutesView";
import { errorMessage, formatDuration } from "../formatters";
import type { Recording } from "../types";

export function LiveRecordingScreen() {
  const [title, setTitle] = useState("");
  const [recording, setRecording] = useState<Recording | null>(null);
  const [isRecording, setIsRecording] = useState(false);
  const [isPending, setIsPending] = useState(false);
  const [startedAt, setStartedAt] = useState<number | null>(null);
  const [elapsedMs, setElapsedMs] = useState(0);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isRecording || startedAt === null) return;

    const updateElapsed = () => setElapsedMs(Date.now() - startedAt);
    updateElapsed();
    const timer = window.setInterval(updateElapsed, 250);
    return () => window.clearInterval(timer);
  }, [isRecording, startedAt]);

  const startRecording = async () => {
    const recordingTitle = title.trim() || `새 회의 ${new Date().toLocaleDateString("ko-KR")}`;
    setIsPending(true);
    setError(null);

    try {
      const nextRecording = await invoke<Recording>("start_recording", {
        title: recordingTitle,
      });
      setRecording(nextRecording);
      setTitle(recordingTitle);
      setElapsedMs(0);
      setStartedAt(Date.now());
      setIsRecording(true);
    } catch (invokeError) {
      setError(errorMessage(invokeError));
    } finally {
      setIsPending(false);
    }
  };

  const stopRecording = async () => {
    setIsPending(true);
    setError(null);

    try {
      const completedRecording = await invoke<Recording>("stop_recording");
      setRecording(completedRecording);
      setElapsedMs(completedRecording.duration_ms ?? elapsedMs);
      setIsRecording(false);
      setStartedAt(null);
    } catch (invokeError) {
      setError(errorMessage(invokeError));
    } finally {
      setIsPending(false);
    }
  };

  return (
    <div className="live-screen">
      <section className="transcript-panel">
        <header className="live-header">
          <div className="live-header-id">
            <p className="live-eyebrow">실시간 세션</p>
            <h1 className="live-title">{recording?.title ?? "새 회의 녹음"}</h1>
          </div>
          <Badge variant={isRecording ? "destructive" : "neutral"} dot pulse={isRecording}>
            {isRecording ? "녹음 중" : "정지"}
          </Badge>
        </header>

        <div className="live-body ds-scroll">
          <section className="live-meter" aria-label="입력 모니터">
            <div className="live-clock">
              <p className="live-field-label">경과 시간</p>
              <time className="live-timer" data-numeric>
                {formatDuration(elapsedMs)}
              </time>
            </div>
            <div className="live-signal">
              <WavVisualizer
                active={isRecording}
                label={isRecording ? "입력 레벨 (신호 대기 중)" : "입력 레벨 (정지)"}
              />
              <p className="live-hint">
                {isRecording
                  ? "회의 내용을 안전하게 녹음하고 있습니다."
                  : recording
                    ? "녹음이 저장되었습니다. 히스토리에서 전사를 시작할 수 있습니다."
                    : "회의 제목을 입력하고 녹음을 시작하세요."}
              </p>
            </div>
          </section>

          <div className="live-controls">
            <label className="title-field">
              <span className="live-field-label">회의 제목</span>
              <input
                value={title}
                onChange={(event) => setTitle(event.currentTarget.value)}
                placeholder="예: 3분기 로드맵 회의"
                disabled={isRecording || isPending}
              />
            </label>
            <Button
              variant={isRecording ? "outline" : "primary"}
              className="record-button"
              onClick={isRecording ? stopRecording : startRecording}
              disabled={isPending}
            >
              <span
                aria-hidden="true"
                className={cn("record-button-icon", isRecording && "stop")}
              />
              {isPending ? "처리 중..." : isRecording ? "녹음 정지" : "녹음 시작"}
            </Button>
          </div>

          {error && (
            <p className="error-banner" role="alert">
              녹음 요청을 처리하지 못했습니다: {error}
            </p>
          )}

          <section className="live-stream" aria-label="실시간 전사 스트림">
            <header className="live-stream-header">
              <div className="live-stream-id">
                <h2 className="live-stream-title">실시간 전사 스트림</h2>
                <p className="live-hint">
                  {isRecording
                    ? "음성을 듣고 있습니다. 전사는 녹음 종료 후 히스토리에서 생성할 수 있습니다."
                    : "녹음을 시작하면 세션 상태가 여기에 표시됩니다."}
                </p>
              </div>
              <Badge variant="partial">확정 대기</Badge>
            </header>
            <div className="live-stream-body">
              <CaptionTimeline
                durationMs={elapsedMs}
                cursorMs={isRecording ? elapsedMs : null}
                emptyLabel={isRecording ? "자막 수신 대기 중" : "자막 없음"}
              />
              <SpeakerTracks
                durationMs={elapsedMs}
                cursorMs={isRecording ? elapsedMs : null}
                emptyLabel="화자 분리 결과 없음"
              />
            </div>
          </section>
        </div>
      </section>
      <MinutesView compact />
    </div>
  );
}
