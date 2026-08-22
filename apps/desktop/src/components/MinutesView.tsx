import { useState, type SyntheticEvent } from "react";
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

type EditableSection = "summary" | "decisions" | "action_items";

interface MinutesViewProps {
  minutes?: MinutesDraft;
  compact?: boolean;
}

interface SectionHeaderProps {
  title: string;
  count?: number;
  onEdit: () => void;
}

function SectionHeader({ title, count, onEdit }: SectionHeaderProps) {
  return (
    <div className="minutes-section-heading">
      <div>
        <h3>{title}</h3>
        {count !== undefined && <span>{count}</span>}
      </div>
      <button type="button" className="text-button" onClick={onEdit}>
        자연어로 수정
      </button>
    </div>
  );
}

function EvidenceBadge({ item }: { item: MinutesItem }) {
  return (
    <span className="evidence-badge">
      근거 {item.evidence_segment_ids.length}개
    </span>
  );
}

export function MinutesView({ minutes = MOCK_MINUTES, compact = false }: MinutesViewProps) {
  const [editing, setEditing] = useState<EditableSection | null>(null);
  const [instruction, setInstruction] = useState("");

  const startEditing = (section: EditableSection) => {
    setEditing(section);
    setInstruction("");
  };

  const submitInstruction = (event: SyntheticEvent<HTMLFormElement, SubmitEvent>) => {
    event.preventDefault();
    const trimmedInstruction = instruction.trim();
    if (!editing || !trimmedInstruction) return;

    console.log("edit_minutes_section", {
      recordingId: minutes.recording_id,
      section: editing,
      instruction: trimmedInstruction,
    });
    // TODO: Connect invoke("edit_minutes_section", ...) when the backend command is available.
    setEditing(null);
    setInstruction("");
  };

  const editor = editing ? (
    <form className="instruction-editor" onSubmit={submitInstruction}>
      <label htmlFor="minutes-instruction">어떻게 바꿀까요?</label>
      <textarea
        id="minutes-instruction"
        autoFocus
        value={instruction}
        onChange={(event) => setInstruction(event.currentTarget.value)}
        placeholder="예: 결정사항을 더 간결하게 정리해줘"
        rows={3}
      />
      <div className="instruction-actions">
        <button type="button" className="button secondary" onClick={() => setEditing(null)}>
          취소
        </button>
        <button type="submit" className="button primary" disabled={!instruction.trim()}>
          수정 요청
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
        <span className="draft-badge">예시 데이터</span>
      </header>

      <div className="minutes-scroll">
        <section className="minutes-section">
          <SectionHeader title="요약" onEdit={() => startEditing("summary")} />
          <button className="summary-card editable-card" type="button" onClick={() => startEditing("summary")}>
            {minutes.summary}
          </button>
          {editing === "summary" && editor}
        </section>

        <section className="minutes-section">
          <SectionHeader
            title="결정 사항"
            count={minutes.decisions.length}
            onEdit={() => startEditing("decisions")}
          />
          <div className="minutes-card-list">
            {minutes.decisions.map((item) => (
              <button
                className="minutes-item decision editable-card"
                type="button"
                key={item.id}
                onClick={() => startEditing("decisions")}
              >
                <span className="item-marker">✓</span>
                <span className="item-copy">{item.text}</span>
                <EvidenceBadge item={item} />
              </button>
            ))}
          </div>
          {editing === "decisions" && editor}
        </section>

        <section className="minutes-section">
          <SectionHeader
            title="할 일"
            count={minutes.action_items.length}
            onEdit={() => startEditing("action_items")}
          />
          <div className="minutes-card-list">
            {minutes.action_items.map((item, index) => (
              <button
                className="minutes-item action editable-card"
                type="button"
                key={item.id}
                onClick={() => startEditing("action_items")}
              >
                <span className="item-number">{String(index + 1).padStart(2, "0")}</span>
                <span className="item-copy">{item.text}</span>
                <EvidenceBadge item={item} />
              </button>
            ))}
          </div>
          {editing === "action_items" && editor}
        </section>
      </div>
    </section>
  );
}
