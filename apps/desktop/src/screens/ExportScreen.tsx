/** S7 — Export. DOCX/PDF are shown per the mockup but disabled — the backend
 * only implements SRT/VTT/MD/TXT so far (see appClient.exportTranscript). */
import { useState } from "react";
import { Button } from "@/components/ui";
import { appClient, type ExportFormat } from "@/platform/appClient";
import { errorMessage } from "../formatters";
import "../styles/export.css";

interface ExportScreenProps {
  recordingId: string;
}

const FORMATS: { value: ExportFormat; label: string; desc: string; enabled: true }[] = [
  { value: "srt", label: "SRT", desc: "자막 파일 · 타임스탬프", enabled: true },
  { value: "vtt", label: "VTT", desc: "웹 자막 · 브라우저", enabled: true },
  { value: "md", label: "MD", desc: "마크다운 문서", enabled: true },
  { value: "txt", label: "TXT", desc: "순수 텍스트", enabled: true },
];

const COMING_SOON = [
  { label: "DOCX", desc: "워드 문서 · 회의록" },
  { label: "PDF", desc: "공유용 문서" },
];

export function ExportScreen({ recordingId }: ExportScreenProps) {
  const [format, setFormat] = useState<ExportFormat>("srt");
  const [isExporting, setIsExporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [resultPath, setResultPath] = useState<string | null>(null);

  const handleExport = async () => {
    setIsExporting(true);
    setError(null);
    setResultPath(null);
    try {
      const path = await appClient.exportTranscript(recordingId, format);
      setResultPath(path);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setIsExporting(false);
    }
  };

  return (
    <section className="export-screen">
      <header className="export-header">
        <p className="export-eyebrow">EXPORT</p>
        <h1>내보내기</h1>
      </header>

      <div className="export-body">
        <div className="export-formats">
          {FORMATS.map((f) => (
            <button
              key={f.value}
              type="button"
              className="export-format"
              aria-pressed={format === f.value}
              onClick={() => {
                setFormat(f.value);
                setResultPath(null);
              }}
            >
              <span className="export-format-icon">{f.label}</span>
              <span className="export-format-main">
                <span className="export-format-name">{f.label}</span>
                <span className="export-format-desc">{f.desc}</span>
              </span>
              <span className="export-format-dot" aria-hidden="true">
                {format === f.value ? "◉" : "○"}
              </span>
            </button>
          ))}
          {COMING_SOON.map((f) => (
            <button key={f.label} type="button" className="export-format" disabled title="준비 중">
              <span className="export-format-icon">{f.label}</span>
              <span className="export-format-main">
                <span className="export-format-name">{f.label}</span>
                <span className="export-format-desc">준비 중</span>
              </span>
            </button>
          ))}
        </div>

        <div className="export-preview">
          <div className="export-preview-label">미리보기 · result.{format}</div>
          <div>
            1<br />
            00:00:32,100 --&gt; 00:00:38,400
            <br />
            박서연: 지난 분기 결제 실패율이 3.2%까지 올랐습니다.
          </div>
        </div>

        {error && (
          <div className="error-banner" role="alert">
            내보내기 실패: {error}
          </div>
        )}

        <div className="export-actions">
          <Button onClick={() => void handleExport()} disabled={isExporting}>
            {isExporting ? "내보내는 중..." : "내보내기"}
          </Button>
        </div>

        {resultPath && (
          <div className="export-result" role="status">
            저장 완료: <code>{resultPath}</code>
          </div>
        )}
      </div>
    </section>
  );
}
