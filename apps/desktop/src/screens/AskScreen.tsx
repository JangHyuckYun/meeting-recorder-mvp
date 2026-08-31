import { useState } from "react";
import { Badge, Button } from "@/components/ui";
import { appClient, type AskNoteResult } from "@/platform/appClient";
import { errorMessage } from "../formatters";
import "../styles/ask.css";

interface AskScreenProps {
  recordingId: string;
}

interface ChatTurn {
  question: string;
  answer: string;
  sources: AskNoteResult["sources"];
}

const SUGGESTIONS = ["결정사항만 요약", "내 액션 아이템은?", "화자별 발언 정리", "마감 기한 정리"];

function formatTimestamp(milliseconds: number): string {
  const minutes = Math.floor(milliseconds / 60_000);
  const seconds = Math.floor(milliseconds / 1_000) % 60;
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}

export function AskScreen({ recordingId }: AskScreenProps) {
  const [question, setQuestion] = useState("");
  const [turns, setTurns] = useState<ChatTurn[]>([]);
  const [isAsking, setIsAsking] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const ask = async (text: string) => {
    const trimmed = text.trim();
    if (!trimmed || isAsking) return;
    setIsAsking(true);
    setError(null);
    setQuestion("");
    try {
      const result = await appClient.askNote(recordingId, trimmed);
      setTurns((prev) => [...prev, { question: trimmed, answer: result.answer, sources: result.sources }]);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setIsAsking(false);
    }
  };

  return (
    <section className="ask-screen">
      <header className="ask-header">
        <p className="ask-eyebrow">ASK</p>
        <h1>질문하기</h1>
      </header>

      <div className="ask-body">
        <nav className="ask-suggestions" aria-label="추천 질문">
          <div className="ask-suggestions-label">추천 질문</div>
          {SUGGESTIONS.map((s) => (
            <button key={s} type="button" className="ask-suggestion" onClick={() => void ask(s)}>
              {s}
            </button>
          ))}
        </nav>

        <div className="ask-panel">
          <div className="ask-thread ds-scroll">
            {turns.length === 0 && !isAsking && (
              <p className="ask-thread-empty">이 노트에 대해 무엇이든 물어보세요.</p>
            )}
            {turns.map((turn, i) => (
              <div key={i} className="ask-turn">
                <div className="ask-bubble-question">{turn.question}</div>
                <div className="ask-bubble-answer">
                  <div className="ask-bubble-answer-text">{turn.answer}</div>
                  {turn.sources.length > 0 && (
                    <div className="ask-sources">
                      {turn.sources.map((source) => (
                        <span key={source.segment_id} className="ask-source-chip" data-numeric>
                          근거 · {formatTimestamp(source.start_ms)}
                        </span>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            ))}
            {isAsking && <Badge variant="neutral">답변 생성 중...</Badge>}
          </div>

          {error && (
            <div className="error-banner" role="alert">
              {error}
            </div>
          )}

          <form
            className="ask-form"
            onSubmit={(e) => {
              e.preventDefault();
              void ask(question);
            }}
          >
            <input
              className="ask-input"
              value={question}
              onChange={(e) => setQuestion(e.target.value)}
              placeholder="이 노트에 대해 무엇이든 물어보세요"
              disabled={isAsking}
            />
            <Button type="submit" disabled={isAsking || !question.trim()}>
              전송
            </Button>
          </form>
        </div>
      </div>
    </section>
  );
}
