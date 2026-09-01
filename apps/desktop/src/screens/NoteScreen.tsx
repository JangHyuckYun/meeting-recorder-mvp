/** S4 — Note detail. 2-view tabs (one-page doc / conversation),
 * speaker rename, minutes with decisions/action-items + evidence, and
 * Export/Ask/Share actions. Contract: NoteScreen({ recordingId, onExport, onAsk, onShare }). */
import { useEffect, useMemo, useState } from "react";
import { Button, Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui";
import { appClient } from "@/platform/appClient";
import type { Template } from "@/platform/appClient";
import { MinutesView } from "../components/MinutesView";
import { errorMessage, formatDate, formatDuration, STATUS_LABELS } from "../formatters";
import "../styles/note.css";
import type { MinutesDraft, Recording, TranscriptSegment } from "../types";

interface NoteScreenProps {
  recordingId: string;
  /** Navigates to S7/S9/S10 with this note's recordingId. */
  onExport: (recordingId: string) => void;
  onAsk: (recordingId: string) => void;
  onShare: (recordingId: string) => void;
}

interface RecordingDetail {
  recording: Recording;
  segments: TranscriptSegment[];
}

type NoteTab = "doc" | "conversation";

export function NoteScreen({ recordingId, onExport, onAsk, onShare }: NoteScreenProps) {
  const [detail, setDetail] = useState<RecordingDetail | null>(null);
  const [minutesDraft, setMinutesDraft] = useState<MinutesDraft | null>(null);
  const [speakerNames, setSpeakerNames] = useState<Record<string, string>>({});
  const [templates, setTemplates] = useState<Template[]>([]);
  const [selectedTemplateId, setSelectedTemplateId] = useState("");
  const [isLoading, setIsLoading] = useState(true);
  const [isGeneratingMinutes, setIsGeneratingMinutes] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<NoteTab>("doc");
  const [pendingScroll, setPendingScroll] = useState<string | null>(null);
  const [editingSpeaker, setEditingSpeaker] = useState<string | null>(null);
  const [speakerInput, setSpeakerInput] = useState("");

  useEffect(() => {
    let cancelled = false;
    setIsLoading(true);
    setError(null);
    setMinutesDraft(null);
    setDetail(null);
    setTemplates([]);
    setSelectedTemplateId("");
    setActiveTab("doc");
    (async () => {
      try {
        const [[recording, segments], minutes, names, templates] = await Promise.all([
          appClient.getRecordingDetail(recordingId),
          appClient.getMinutes(recordingId),
          appClient.getSpeakerNames(recordingId),
          appClient.listTemplates(),
        ]);
        if (cancelled) return;
        setDetail({ recording, segments });
        if (minutes) setMinutesDraft(minutes);
        setSpeakerNames(names);
        setTemplates(templates);
      } catch (invokeError) {
        if (!cancelled) setError(errorMessage(invokeError));
      } finally {
        if (!cancelled) setIsLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [recordingId]);

  useEffect(() => {
    if (activeTab !== "conversation" || !pendingScroll) return;
    const el = document.querySelector(`[data-segment-id="${pendingScroll}"]`);
    el?.scrollIntoView({ behavior: "smooth", block: "center" });
    setPendingScroll(null);
  }, [activeTab, pendingScroll]);

  const speakerLabels = useMemo(() => {
    const seen = new Set<string>();
    const ordered: string[] = [];
    for (const segment of detail?.segments ?? []) {
      if (!seen.has(segment.speaker_label)) {
        seen.add(segment.speaker_label);
        ordered.push(segment.speaker_label);
      }
    }
    return ordered;
  }, [detail]);

  const generateMinutes = async () => {
    if (!detail) return;
    setIsGeneratingMinutes(true);
    setError(null);
    try {
      const draft = await appClient.generateMinutes(detail.recording.id, selectedTemplateId || undefined);
      setMinutesDraft(draft);
    } catch (invokeError) {
      setError(errorMessage(invokeError));
    } finally {
      setIsGeneratingMinutes(false);
    }
  };

  const handleItemEdited = (
    section: "decisions" | "action_items",
    itemId: string,
    text: string,
  ) => {
    setMinutesDraft((current) => {
      if (!current) return current;
      return {
        ...current,
        [section]: current[section].map((item) => (item.id === itemId ? { ...item, text } : item)),
      };
    });
  };

  const startRenameSpeaker = (label: string) => {
    setEditingSpeaker(label);
    setSpeakerInput(speakerNames[label] ?? "");
  };

  const saveSpeakerName = async (label: string) => {
    const name = speakerInput.trim();
    try {
      await appClient.setSpeakerName(recordingId, label, name);
      setSpeakerNames((current) => {
        const next = { ...current };
        if (name) next[label] = name;
        else delete next[label];
        return next;
      });
    } catch (invokeError) {
      setError(errorMessage(invokeError));
    } finally {
      setEditingSpeaker(null);
    }
  };

  const jumpToSegment = (segmentId?: string) => {
    if (!segmentId) return;
    setActiveTab("conversation");
    setPendingScroll(segmentId);
  };

  if (isLoading || !detail) {
    return (
      <div className="note-screen">
        <p className="history-loading">
          <span aria-hidden="true" />
          노트를 불러오는 중...
        </p>
      </div>
    );
  }

  const canGenerateMinutes = detail.segments.length > 0 && !minutesDraft;

  return (
    <div className="note-screen">
      <header className="note-header">
        <div className="note-header-title">
          <span className="note-eyebrow">Note</span>
          <div className="note-title-row">
            <h1>{detail.recording.title}</h1>
            <span className="history-status" data-status={detail.recording.status}>
              {STATUS_LABELS[detail.recording.status]}
            </span>
          </div>
        </div>
        <div className="note-header-actions">
          <Button variant="outline" size="sm" onClick={() => onAsk(recordingId)}>
            질문하기
          </Button>
          <Button variant="outline" size="sm" onClick={() => onExport(recordingId)}>
            내보내기
          </Button>
          <Button size="sm" onClick={() => onShare(recordingId)}>
            공유
          </Button>
        </div>
      </header>

      {error && (
        <div className="error-banner" role="alert">
          요청을 처리하지 못했습니다: {error}
        </div>
      )}

      <div className="note-body">
        <div className="note-main">
          <Tabs
            className="flex min-h-0 flex-1 flex-col"
            value={activeTab}
            onValueChange={(value) => setActiveTab(value as NoteTab)}
          >
            <TabsList>
              <TabsTrigger value="doc">한 페이지 문서</TabsTrigger>
              <TabsTrigger value="conversation">대화 기록(스크립트)</TabsTrigger>
            </TabsList>

            <TabsContent className="note-document ds-scroll" value="doc">
              <p className="note-doc-meta">
                <span data-numeric>{formatDate(detail.recording.created_at)}</span>
                <span aria-hidden="true">·</span>
                <span data-numeric>{formatDuration(detail.recording.duration_ms)}</span>
                <span aria-hidden="true">·</span>
                <span>참석 {speakerLabels.length || 1}인</span>
              </p>
              <label className="note-template-select">
                회의록 템플릿
                <select
                  aria-label="회의록 템플릿"
                  value={selectedTemplateId}
                  onChange={(event) => setSelectedTemplateId(event.currentTarget.value)}
                >
                  <option value="">기본</option>
                  {templates.map((template) => (
                    <option key={template.id} value={template.id}>
                      {template.name}
                    </option>
                  ))}
                </select>
              </label>
              {minutesDraft ? (
                <MinutesView
                  minutes={minutesDraft}
                  recordingId={recordingId}
                  onItemEdited={handleItemEdited}
                />
              ) : (
                <div className="history-note">
                  {canGenerateMinutes ? (
                    <>
                      <h3>아직 회의록이 없습니다</h3>
                      <p>전사 내용을 바탕으로 결정 사항과 할 일을 생성하세요.</p>
                      <Button
                        size="sm"
                        className="history-note-action"
                        disabled={isGeneratingMinutes}
                        onClick={generateMinutes}
                      >
                        {isGeneratingMinutes ? "회의록 생성 중..." : "회의록 생성"}
                      </Button>
                    </>
                  ) : (
                    <>
                      <h3>전사가 먼저 필요합니다</h3>
                      <p>전사를 먼저 완료한 뒤 회의록을 만들 수 있습니다.</p>
                    </>
                  )}
                </div>
              )}
            </TabsContent>

            <TabsContent value="conversation">
              <div className="history-transcript ds-scroll note-conversation">
                {detail.segments.length > 0 ? (
                  detail.segments.map((segment, index) => (
                    <article className="history-segment" key={segment.id} data-segment-id={segment.id}>
                      <time className="history-segment-time" data-numeric>
                        {formatDuration(segment.start_ms)}
                      </time>
                      <div>
                        <div className="history-segment-meta">
                          <span className="history-speaker" data-speaker={index % 4}>
                            {speakerNames[segment.speaker_label] ?? segment.speaker_label}
                          </span>
                          {!segment.is_final && <span className="history-segment-flag">처리 중</span>}
                        </div>
                        <p>{segment.text}</p>
                      </div>
                    </article>
                  ))
                ) : (
                  <div className="history-note">
                    <h3>아직 전사 내용이 없습니다</h3>
                    <p>전사를 먼저 완료하면 대화 기록을 볼 수 있습니다.</p>
                  </div>
                )}
              </div>
            </TabsContent>

          </Tabs>
        </div>

        <aside className="note-side">
          <section className="note-card">
            <h3 className="note-card-title">화자 → 실명 매핑</h3>
            {speakerLabels.length === 0 ? (
              <p className="note-card-empty">아직 화자 정보가 없습니다.</p>
            ) : (
              speakerLabels.map((label) => (
                <div className="note-speaker-row" key={label}>
                  <span className="history-speaker">{label}</span>
                  {editingSpeaker === label ? (
                    <input
                      autoFocus
                      className="note-speaker-input"
                      value={speakerInput}
                      onChange={(event) => setSpeakerInput(event.currentTarget.value)}
                      onBlur={() => void saveSpeakerName(label)}
                      onKeyDown={(event) => {
                        if (event.key === "Enter") void saveSpeakerName(label);
                        if (event.key === "Escape") setEditingSpeaker(null);
                      }}
                    />
                  ) : (
                    <button
                      type="button"
                      className="note-speaker-edit"
                      onClick={() => startRenameSpeaker(label)}
                    >
                      {speakerNames[label] ?? "이름 지정"} ✎
                    </button>
                  )}
                </div>
              ))
            )}
          </section>

          {minutesDraft && (minutesDraft.decisions.length > 0 || minutesDraft.action_items.length > 0) && (
            <section className="note-card">
              <h3 className="note-card-title">문단 다시듣기</h3>
              {minutesDraft.decisions.map((item) => (
                <button
                  type="button"
                  className="note-jump-row"
                  key={item.id}
                  onClick={() => jumpToSegment(item.evidence_segment_ids[0])}
                >
                  <span aria-hidden="true">▶</span>
                  <span>결정사항</span>
                </button>
              ))}
              {minutesDraft.action_items.map((item) => (
                <button
                  type="button"
                  className="note-jump-row"
                  key={item.id}
                  onClick={() => jumpToSegment(item.evidence_segment_ids[0])}
                >
                  <span aria-hidden="true">▶</span>
                  <span>액션아이템</span>
                </button>
              ))}
            </section>
          )}
        </aside>
      </div>
    </div>
  );
}
