import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui";
import { appClient, type TranscriptionProgressEvent } from "@/platform/appClient";
import { formatRemainingMs, estimateRemainingMs } from "../lib/transcriptionEta";
import { importStore, useImportJob } from "../state/importStore";
import { errorMessage } from "../formatters";
import type { Recording } from "../types";

const ACCEPTED_EXTENSIONS = ["m4a", "wav", "mp3", "mp4", "aac"];
const LANGUAGES = ["한국어", "영어", "다국어"];

const PHASE_LABELS: Record<TranscriptionProgressEvent["phase"], string> = {
  sending: "업로드 · 변환 중",
  finalizing: "화자분리 · 요약 중",
  done: "완료",
};

export function ImportScreen() {
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [title, setTitle] = useState("");
  const [isImporting, setIsImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [localSuccess, setLocalSuccess] = useState<Recording | null>(null);
  const [speakerCount, setSpeakerCount] = useState("auto");
  const [language, setLanguage] = useState(LANGUAGES[0]);
  const [isTranscribing, setIsTranscribing] = useState(false);
  const [transcribeError, setTranscribeError] = useState<string | null>(null);
  const importJob = useImportJob();
  const success = importJob?.recording ?? localSuccess;
  const progress = importJob?.progress ?? null;
  const jobIsTranscribing = !!importJob && importJob.progress.phase !== "done";

  useEffect(() => {
    appClient
      .getAppSettings()
      .then((settings) => setSpeakerCount(settings.speakers?.toString() ?? "auto"))
      .catch(() => {});
  }, []);

  const pickFile = async () => {
    setError(null);
    setLocalSuccess(null);
    try {
      const picked = await open({
        multiple: false,
        filters: [{ name: "오디오 파일", extensions: ACCEPTED_EXTENSIONS }],
      });
      if (typeof picked === "string" && picked) {
        setSelectedPath(picked);
      }
    } catch (err) {
      setError(errorMessage(err));
    }
  };

  const importFile = async () => {
    if (!selectedPath) return;
    setIsImporting(true);
    setError(null);
    setLocalSuccess(null);
    try {
      const importedTitle = title.trim() || selectedPath.split("/").pop() || selectedPath;
      const recording = await appClient.ingestAudioFile(selectedPath, importedTitle);
      setLocalSuccess(recording);
      setSelectedPath(null);
      setTitle("");
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setIsImporting(false);
    }
  };

  const startTranscription = async () => {
    if (!success) return;
    setIsTranscribing(true);
    setTranscribeError(null);
    importStore.start(success);
    try {
      await appClient.transcribeRecording(
        success.id,
        speakerCount === "auto" ? null : Number(speakerCount),
      );
    } catch (err) {
      setTranscribeError(errorMessage(err));
    } finally {
      setIsTranscribing(false);
    }
  };

  const cancelTranscription = async () => {
    try {
      await appClient.cancelTranscription();
    } finally {
      importStore.reset();
      setIsTranscribing(false);
    }
  };

  const fileName = selectedPath ? selectedPath.split("/").pop() ?? selectedPath : null;
  const resolvedTitle = title.trim() || fileName;

  return (
    <section className="import-screen">
      <header className="import-header">
        <p className="import-eyebrow">IMPORT</p>
        <h1>가져오기</h1>
        <p className="import-lede">
          기존 녹음 파일을 보관함으로 옮깁니다. 가져온 뒤 히스토리 탭에서 전사를 시작할 수 있습니다.
        </p>
      </header>

      <div className="import-body">
        <div className="import-form">
          <div className="import-row">
            <span className="import-step" data-numeric aria-hidden="true">
              01
            </span>
            <div className="import-row-main">
              <span className="import-label" id="import-file-label">
                오디오 파일
              </span>
              <p
                className="import-path"
                id="import-file-path"
                data-empty={fileName ? undefined : ""}
                aria-live="polite"
              >
                {fileName ?? "선택된 파일 없음"}
              </p>
              <p className="import-hint">
                지원 형식
                <span data-numeric>{ACCEPTED_EXTENSIONS.join(" · ")}</span>
              </p>
            </div>
            <Button
              variant="outline"
              size="sm"
              className="import-pick"
              onClick={pickFile}
              disabled={isImporting}
              aria-labelledby="import-file-label"
              aria-describedby="import-file-path"
            >
              파일 선택
            </Button>
          </div>

          <div className="import-row">
            <span className="import-step" data-numeric aria-hidden="true">
              02
            </span>
            <div className="import-row-main">
              <label className="import-label" htmlFor="import-title">
                회의 제목
                <span className="import-optional">선택</span>
              </label>
              <input
                id="import-title"
                className="import-input"
                value={title}
                onChange={(e) => setTitle(e.currentTarget.value)}
                placeholder="예: 3분기 로드맵 회의"
                disabled={isImporting}
              />
              <p className="import-hint">
                비워두면 파일 이름을 제목으로 사용합니다.
              </p>
            </div>
          </div>
        </div>

        <div className="import-actions">
          <p className="import-summary">
            {resolvedTitle ? (
              <>
                저장될 제목 <strong>{resolvedTitle}</strong>
              </>
            ) : (
              "파일을 선택하면 가져오기를 시작할 수 있습니다."
            )}
          </p>
          <Button disabled={!selectedPath || isImporting} onClick={importFile} size="sm">
            {isImporting ? "가져오는 중..." : "가져오기"}
          </Button>
        </div>

        {error && (
          <div className="error-banner" role="alert">
            가져오기 실패: {error}
          </div>
        )}
        {success && (
          <div className="success-banner" role="status">
            &quot;{success.title}&quot; 가져오기를 완료했습니다. 이어서 전사를 시작할 수 있습니다.
          </div>
        )}

        {success && (
          <div className="import-transcribe">
            <div className="import-options">
              <div className="import-field">
                <label htmlFor="import-speaker-count">화자 수</label>
                <select
                  id="import-speaker-count"
                  data-numeric
                  value={speakerCount}
                  onChange={(e) => setSpeakerCount(e.currentTarget.value)}
                  disabled={isTranscribing}
                >
                  <option value="auto">자동</option>
                  {Array.from({ length: 9 }, (_, index) => index + 2).map((count) => (
                    <option key={count} value={count}>
                      {count}명
                    </option>
                  ))}
                </select>
              </div>
              <div className="import-field import-field-grow">
                <label htmlFor="import-language">언어</label>
                <select
                  id="import-language"
                  value={language}
                  onChange={(e) => setLanguage(e.currentTarget.value)}
                  disabled={isTranscribing}
                >
                  {LANGUAGES.map((lang) => (
                    <option key={lang}>{lang}</option>
                  ))}
                </select>
              </div>
            </div>

            {progress && (
              <div className="import-progress">
                <div className="import-progress-head">
                  <b>전사 진행률</b>
                  <span data-numeric>
                    {progress.total_ms > 0
                      ? Math.round((progress.sent_ms / progress.total_ms) * 100)
                      : 0}
                    %
                  </span>
                </div>
                <div className="import-progress-track">
                  <div
                    className="import-progress-fill"
                    style={{
                      width: `${
                        progress.total_ms > 0
                          ? Math.min(100, (progress.sent_ms / progress.total_ms) * 100)
                          : 0
                      }%`,
                    }}
                  />
                </div>
                <span className="import-progress-phase">{PHASE_LABELS[progress.phase]}</span>
                {importJob && (
                  <span className="import-progress-eta">
                    {formatRemainingMs(
                      estimateRemainingMs(importJob.samples, Date.now()),
                    )}
                  </span>
                )}
              </div>
            )}

            {transcribeError && (
              <div className="error-banner" role="alert">
                전사 실패: {transcribeError}
              </div>
            )}

            <div className="import-actions">
              {isTranscribing || jobIsTranscribing ? (
                <Button variant="outline" size="sm" onClick={cancelTranscription}>
                  전사 취소
                </Button>
              ) : (
                <Button size="sm" onClick={startTranscription}>
                  전사 시작
                </Button>
              )}
            </div>
          </div>
        )}
      </div>
    </section>
  );
}
