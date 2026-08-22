import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { errorMessage } from "../formatters";
import type { Recording } from "../types";

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
        filters: [{ name: "오디오 파일", extensions: ["m4a", "wav", "mp3", "mp4", "aac"] }],
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

  return (
    <section className="import-screen">
      <header className="history-header">
        <div>
          <p className="eyebrow">IMPORT</p>
          <h1>가져오기</h1>
          <p>기존 녹음 파일을 선택해 보관함으로 가져오세요.</p>
        </div>
      </header>

      <div className="import-panel">
        <label>오디오 파일 선택</label>
        <div className="import-fields">
          <button type="button" className="button secondary" onClick={pickFile} disabled={isImporting}>
            파일 선택
          </button>
          <span className="import-selected-path">{fileName ?? "선택된 파일 없음"}</span>
        </div>

        <label htmlFor="import-title">회의 제목 (선택)</label>
        <input
          id="import-title"
          value={title}
          onChange={(e) => setTitle(e.currentTarget.value)}
          placeholder="예: 3분기 로드맵 회의"
          disabled={isImporting}
        />

        <button
          type="button"
          className="button primary"
          disabled={!selectedPath || isImporting}
          onClick={importFile}
        >
          {isImporting ? "가져오는 중..." : "가져오기"}
        </button>

        {error && <div className="error-banner">가져오기 실패: {error}</div>}
        {success && (
          <div className="success-banner">
            &quot;{success.title}&quot; 가져오기를 완료했습니다. 히스토리 탭에서 확인할 수 있습니다.
          </div>
        )}
      </div>
    </section>
  );
}
