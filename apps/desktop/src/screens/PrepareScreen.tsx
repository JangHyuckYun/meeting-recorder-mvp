/** S2 — Prepare recording: title, attendees, context/keyterm glossary, start. */
import { useEffect, useState } from "react";
import { Button } from "@/components/ui";
import { appClient } from "@/platform/appClient";
import { errorMessage } from "../formatters";
import "../styles/prepare.css";

interface PrepareScreenProps {
  /** Called after startRecording(title) succeeds — navigates to S3 with recording running. */
  onStart: () => void;
}

const LANGUAGES = ["한국어", "영어", "일본어", "다국어 (자동감지)"];

export function PrepareScreen({ onStart }: PrepareScreenProps) {
  const [title, setTitle] = useState("");
  // ponytail: attendees have no backend field yet — local-state only, not persisted.
  const [attendees, setAttendees] = useState<string[]>([]);
  const [attendeeDraft, setAttendeeDraft] = useState("");
  const [context, setContext] = useState("");
  // ponytail: language selects have no backend param — local-state only, cosmetic per mockup.
  const [language, setLanguage] = useState(LANGUAGES[0]);
  const [isStarting, setIsStarting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    appClient
      .getGlossary()
      .then((terms) => setContext(terms.join(", ")))
      .catch(() => {
        /* glossary is a nice-to-have; ignore load failure */
      });
  }, []);

  const addAttendee = () => {
    const name = attendeeDraft.trim();
    if (!name) return;
    setAttendees((prev) => (prev.includes(name) ? prev : [...prev, name]));
    setAttendeeDraft("");
  };

  const removeAttendee = (name: string) => {
    setAttendees((prev) => prev.filter((entry) => entry !== name));
  };

  const start = async () => {
    setIsStarting(true);
    setError(null);
    try {
      const terms = context
        .split(/[,\n]/)
        .map((term) => term.trim())
        .filter(Boolean);
      await appClient.setGlossary(terms).catch(() => {
        /* glossary save is best-effort; recording must still start */
      });
      const recordingTitle = title.trim() || `새 회의 ${new Date().toLocaleDateString("ko-KR")}`;
      await appClient.startRecording(recordingTitle);
      onStart();
    } catch (invokeError) {
      setError(errorMessage(invokeError));
    } finally {
      setIsStarting(false);
    }
  };

  return (
    <section className="prepare-screen">
      <header className="prepare-header">
        <p className="prepare-eyebrow">NEW NOTE</p>
        <h1>기록 준비</h1>
        <p className="prepare-lede">시작 전 설정</p>
      </header>

      <div className="prepare-body">
        <div className="prepare-field">
          <label htmlFor="prepare-title">노트 제목</label>
          <input
            id="prepare-title"
            value={title}
            onChange={(event) => setTitle(event.currentTarget.value)}
            placeholder="비우면 자동 생성"
            disabled={isStarting}
          />
        </div>

        <div className="prepare-field">
          <label htmlFor="prepare-attendee">대화 참석자</label>
          <div className="prepare-pills">
            {attendees.map((name) => (
              <button
                key={name}
                type="button"
                className="prepare-pill"
                onClick={() => removeAttendee(name)}
                disabled={isStarting}
              >
                {name} ×
              </button>
            ))}
            <input
              id="prepare-attendee"
              className="prepare-pill-input"
              value={attendeeDraft}
              onChange={(event) => setAttendeeDraft(event.currentTarget.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  addAttendee();
                }
              }}
              onBlur={addAttendee}
              placeholder="+ 이름·이메일 추가"
              disabled={isStarting}
            />
          </div>
        </div>

        <div className="prepare-field">
          <label htmlFor="prepare-context">맥락 입력 (정확도 향상)</label>
          <textarea
            id="prepare-context"
            value={context}
            onChange={(event) => setContext(event.currentTarget.value)}
            placeholder="고유명사·키워드를 쉼표로 구분해 입력하세요"
            rows={4}
            disabled={isStarting}
          />
          <p className="prepare-hint">쉼표로 구분한 용어가 STT 정확도 향상에 사용됩니다.</p>
        </div>

        <div className="prepare-row">
          <div className="prepare-field">
            <label htmlFor="prepare-language">대화 언어</label>
            <select
              id="prepare-language"
              value={language}
              onChange={(event) => setLanguage(event.currentTarget.value)}
              disabled={isStarting}
            >
              {LANGUAGES.map((lang) => (
                <option key={lang}>{lang}</option>
              ))}
            </select>
          </div>
        </div>

        {error && (
          <p className="error-banner" role="alert">
            녹음 시작에 실패했습니다: {error}
          </p>
        )}

        <Button
          variant="destructive"
          className="prepare-start"
          onClick={start}
          disabled={isStarting}
        >
          <span aria-hidden="true" className="record-button-icon" />
          {isStarting ? "시작하는 중..." : "기록 시작"}
        </Button>
      </div>
    </section>
  );
}
