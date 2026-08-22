import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
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
        <header className="panel-header live-heading">
          <div>
            <p className="eyebrow">LIVE SESSION</p>
            <h1>{recording?.title ?? "새 회의 녹음"}</h1>
          </div>
          <span className={`status-pill ${isRecording ? "recording" : "idle"}`}>
            <span className="status-dot" />
            {isRecording ? "녹음 중" : "정지"}
          </span>
        </header>

        <div className="recording-hero">
          <div className={`waveform ${isRecording ? "active" : ""}`} aria-hidden="true">
            {[18, 34, 52, 30, 68, 42, 76, 46, 60, 26, 48, 72, 38, 56, 24, 44, 64, 32].map(
              (height, index) => (
                <span key={index} style={{ height }} />
              ),
            )}
          </div>
          <time className="recording-time">{formatDuration(elapsedMs)}</time>
          <p>
            {isRecording
              ? "회의 내용을 안전하게 녹음하고 있습니다."
              : recording
                ? "녹음이 저장되었습니다. 히스토리에서 전사를 시작할 수 있습니다."
                : "회의 제목을 입력하고 녹음을 시작하세요."}
          </p>
        </div>

        <div className="recording-controls">
          <label className="title-field">
            <span>회의 제목</span>
            <input
              value={title}
              onChange={(event) => setTitle(event.currentTarget.value)}
              placeholder="예: 3분기 로드맵 회의"
              disabled={isRecording || isPending}
            />
          </label>
          <button
            type="button"
            className={`record-button ${isRecording ? "stop" : "start"}`}
            onClick={isRecording ? stopRecording : startRecording}
            disabled={isPending}
          >
            <span className="record-button-icon" />
            {isPending ? "처리 중..." : isRecording ? "녹음 정지" : "녹음 시작"}
          </button>
        </div>

        {error && <div className="error-banner">녹음 요청을 처리하지 못했습니다: {error}</div>}

        <div className="live-transcript-placeholder">
          <div className="placeholder-icon">“</div>
          <div>
            <h2>실시간 전사 스트림</h2>
            <p>
              {isRecording
                ? "음성을 듣고 있습니다. 전사는 녹음 종료 후 히스토리에서 생성할 수 있습니다."
                : "녹음을 시작하면 세션 상태가 여기에 표시됩니다."}
            </p>
          </div>
        </div>
      </section>
      <MinutesView compact />
    </div>
  );
}
