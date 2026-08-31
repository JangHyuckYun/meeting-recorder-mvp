import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { Button } from "@/components/ui";
import { errorMessage } from "../formatters";
import type { Recording } from "../types";

const ACCEPTED_EXTENSIONS = ["m4a", "wav", "mp3", "mp4", "aac"];

export function ImportScreen() {
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [title, setTitle] = useState("");
  const [isImporting, setIsImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<Recording | null>(null);

  const pickFile = async () => {
    setError(null);
    setSuccess(null);
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
    setSuccess(null);
    try {
      const importedTitle = title.trim() || selectedPath.split("/").pop() || selectedPath;
      const recording = await invoke<Recording>("ingest_audio_file", {
        sourcePath: selectedPath,
        title: importedTitle,
      });
      setSuccess(recording);
      setSelectedPath(null);
      setTitle("");
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setIsImporting(false);
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
            &quot;{success.title}&quot; 가져오기를 완료했습니다. 히스토리 탭에서 확인할 수 있습니다.
          </div>
        )}
      </div>
    </section>
  );
}
