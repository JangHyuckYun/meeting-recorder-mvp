import { useState, type SyntheticEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { errorMessage } from "../formatters";
import type { MinutesDraft, MinutesItem } from "../types";

export const MOCK_MINUTES: MinutesDraft = {
  recording_id: "mock-recording-01",
  summary:
    "3분기 제품 로드맵의 우선순위를 확정하고 마케팅 캠페인과 디자인 시안의 다음 일정을 조율했습니다.",
  decisions: [
    {
      id: "decision-01",
      text: "3분기 로드맵 방향과 핵심 우선순위를 이번 주 안에 확정합니다.",
      evidence_segment_ids: ["segment-01", "segment-03"],
    },
    {
      id: "decision-02",
      text: "마케팅 캠페인은 7월 둘째 주에 론칭합니다.",
      evidence_segment_ids: ["segment-02"],
    },
  ],
  action_items: [
    {
      id: "action-01",
      text: "콘텐츠 및 광고 예산 조정안 작성",
      evidence_segment_ids: ["segment-02"],
    },
    {
      id: "action-02",
      text: "디자인 시안 1차 버전을 목요일까지 공유",
      evidence_segment_ids: ["segment-03"],
    },
  ],
};

type ItemSection = "decisions" | "action_items";

interface MinutesViewProps {
  minutes?: MinutesDraft;
  compact?: boolean;
  /**
   * Real backend recording id. When set, clicking an item calls the live
   * `edit_minutes_item` command against that recording instead of the mock no-op stub.
   * Must only be passed together with a real (non-mock) `minutes` draft — mock item ids
   * do not exist on the backend and an edit against them will fail with "not found".
   */
  recordingId?: string;
  /** Called with the freshly edited text once the backend confirms the edit. */
  onItemEdited?: (section: ItemSection, itemId: string, text: string) => void;
}

interface EditTarget {
  section: ItemSection;
  item: MinutesItem;
}

function EvidenceBadge({ item }: { item: MinutesItem }) {
  return <span className="evidence-badge">근거 {item.evidence_segment_ids.length}개</span>;
}

export function MinutesView({ minutes = MOCK_MINUTES, compact = false, recordingId, onItemEdited }: MinutesViewProps) {
  const [editing, setEditing] = useState<EditTarget | null>(null);
  const [instruction, setInstruction] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [editError, setEditError] = useState<string | null>(null);

  const startEditing = (section: ItemSection, item: MinutesItem) => {
    setEditing({ section, item });
    setInstruction("");
    setEditError(null);
  };

  const submitInstruction = async (event: SyntheticEvent<HTMLFormElement, SubmitEvent>) => {
    event.preventDefault();
    const trimmedInstruction = instruction.trim();
    if (!editing || !trimmedInstruction) return;

    if (!recordingId) {
      // Mock/demo mode (실시간 화면 미리보기 등): no real recording to edit against.
      console.log("edit_minutes_item (mock, no-op)", {
        section: editing.section,
        itemId: editing.item.id,
        instruction: trimmedInstruction,
      });
      setEditing(null);
      setInstruction("");
      return;
    }

    setIsSubmitting(true);
    setEditError(null);
    try {
      const updated = await invoke<MinutesItem>("edit_minutes_item", {
        recordingId,
        itemId: editing.item.id,
        instruction: trimmedInstruction,
      });
      onItemEdited?.(editing.section, editing.item.id, updated.text);
      setEditing(null);
      setInstruction("");
    } catch (invokeError) {
      setEditError(errorMessage(invokeError));
    } finally {
      setIsSubmitting(false);
    }
  };

  const editor = editing ? (
    <form className="instruction-editor" onSubmit={submitInstruction}>
      <label htmlFor="minutes-instruction">어떻게 바꿀까요?</label>
      <textarea
        id="minutes-instruction"
        autoFocus
        value={instruction}
        onChange={(event) => setInstruction(event.currentTarget.value)}
        placeholder="예: 더 간결하고 격식 있게 다듬어줘"
        rows={3}
      />
      {editError && <p className="error-banner">수정 요청 실패: {editError}</p>}
      <div className="instruction-actions">
        <button
          type="button"
          className="button secondary"
          onClick={() => setEditing(null)}
          disabled={isSubmitting}
        >
          취소
        </button>
        <button type="submit" className="button primary" disabled={!instruction.trim() || isSubmitting}>
          {isSubmitting ? "수정 중..." : "수정 요청"}
        </button>
      </div>
    </form>
  ) : null;

  return (
    <section className={`minutes-view ${compact ? "compact" : ""}`} aria-label="회의록 초안">
      <header className="panel-header">
        <div>
          <p className="eyebrow">AI MEETING NOTES</p>
          <h2>회의록 초안</h2>
        </div>
        <span className="draft-badge">{recordingId ? "AI 생성" : "예시 데이터"}</span>
      </header>

      <div className="minutes-scroll">
        <section className="minutes-section">
          <div className="minutes-section-heading">
            <div>
              <h3>요약</h3>
            </div>
          </div>
          <p className="summary-card">{minutes.summary}</p>
        </section>

        <section className="minutes-section">
          <div className="minutes-section-heading">
            <div>
              <h3>결정 사항</h3>
              <span>{minutes.decisions.length}</span>
            </div>
          </div>
          <div className="minutes-card-list">
            {minutes.decisions.map((item) => (
              <button
                className="minutes-item decision editable-card"
                type="button"
                key={item.id}
                onClick={() => startEditing("decisions", item)}
              >
                <span className="item-marker">✓</span>
                <span className="item-copy">{item.text}</span>
                <EvidenceBadge item={item} />
              </button>
            ))}
          </div>
          {editing?.section === "decisions" && editor}
        </section>

        <section className="minutes-section">
          <div className="minutes-section-heading">
            <div>
              <h3>할 일</h3>
              <span>{minutes.action_items.length}</span>
            </div>
          </div>
          <div className="minutes-card-list">
            {minutes.action_items.map((item, index) => (
              <button
                className="minutes-item action editable-card"
                type="button"
                key={item.id}
                onClick={() => startEditing("action_items", item)}
              >
                <span className="item-number">{String(index + 1).padStart(2, "0")}</span>
                <span className="item-copy">{item.text}</span>
                <EvidenceBadge item={item} />
              </button>
            ))}
          </div>
          {editing?.section === "action_items" && editor}
        </section>
      </div>
    </section>
  );
}
