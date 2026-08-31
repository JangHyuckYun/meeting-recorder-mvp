/** S5 — Templates. Preset templates are static/read-only; custom templates are
 * CRUD-backed by appClient (listTemplates/createTemplate/updateTemplate/deleteTemplate). */
import { useEffect, useState } from "react";
import { Badge, Button } from "@/components/ui";
import { appClient, type Template } from "@/platform/appClient";
import { errorMessage } from "../formatters";
import "../styles/templates.css";

interface PresetTemplate {
  id: string;
  name: string;
  content: string;
}

const PRESET_TEMPLATES: PresetTemplate[] = [
  { id: "preset-minutes", name: "회의록", content: "회의 개요 (제목·일시·참석자)\n결정사항 (담당·기한 포함)\n주요 논의\n액션 아이템 [담당 | 내용 | 기한]" },
  { id: "preset-interview", name: "인터뷰", content: "인터뷰 개요 (일시·대상자)\n주요 질문과 답변\n핵심 인용\n후속 조치" },
  { id: "preset-lecture", name: "강의 노트", content: "강의 개요 (주제·강사·일시)\n핵심 개념\n예시 및 사례\n복습 질문" },
  { id: "preset-standup", name: "스탠드업", content: "일자\n어제 한 일\n오늘 할 일\n블로커" },
  { id: "preset-1on1", name: "1:1 미팅", content: "일자·참석자\n논의 사항\n피드백\n다음 액션" },
];

type Selection = { kind: "preset"; template: PresetTemplate } | { kind: "custom"; template: Template } | { kind: "new" };

export function TemplatesScreen() {
  const [customTemplates, setCustomTemplates] = useState<Template[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selection, setSelection] = useState<Selection>({ kind: "preset", template: PRESET_TEMPLATES[0] });
  const [name, setName] = useState("");
  const [content, setContent] = useState("");
  const [nameError, setNameError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);

  const load = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const templates = await appClient.listTemplates();
      setCustomTemplates(templates);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    void load();
  }, []);

  useEffect(() => {
    if (selection.kind === "preset") {
      setName(selection.template.name);
      setContent(selection.template.content);
    } else if (selection.kind === "custom") {
      setName(selection.template.name);
      setContent(selection.template.content);
    } else {
      setName("");
      setContent("");
    }
    setNameError(null);
  }, [selection]);

  const isReadOnly = selection.kind === "preset";

  const handleSave = async () => {
    const trimmed = name.trim();
    if (!trimmed) {
      setNameError("템플릿 이름을 입력하세요.");
      return;
    }
    setIsSaving(true);
    setError(null);
    try {
      if (selection.kind === "new") {
        const created = await appClient.createTemplate(trimmed, content);
        setCustomTemplates((prev) => [...prev, created]);
        setSelection({ kind: "custom", template: created });
      } else if (selection.kind === "custom") {
        await appClient.updateTemplate(selection.template.id, trimmed, content);
        const updated = { ...selection.template, name: trimmed, content };
        setCustomTemplates((prev) => prev.map((t) => (t.id === updated.id ? updated : t)));
        setSelection({ kind: "custom", template: updated });
      }
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setIsSaving(false);
    }
  };

  const handleDelete = async () => {
    if (selection.kind !== "custom") return;
    setIsSaving(true);
    setError(null);
    try {
      await appClient.deleteTemplate(selection.template.id);
      setCustomTemplates((prev) => prev.filter((t) => t.id !== selection.template.id));
      setSelection({ kind: "preset", template: PRESET_TEMPLATES[0] });
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <section className="templates-screen">
      <header className="templates-header">
        <p className="templates-eyebrow">TEMPLATES</p>
        <h1>템플릿</h1>
        <p className="templates-lede">원하는 양식으로 3초 만에 회의록을 정리하세요.</p>
      </header>

      <div className="templates-body">
        <nav className="templates-list" aria-label="템플릿 목록">
          <div className="templates-list-label">프리셋 템플릿</div>
          {PRESET_TEMPLATES.map((preset) => (
            <button
              key={preset.id}
              type="button"
              className="templates-list-item"
              aria-current={selection.kind === "preset" && selection.template.id === preset.id}
              onClick={() => setSelection({ kind: "preset", template: preset })}
            >
              <span>{preset.name}</span>
            </button>
          ))}
          {customTemplates.map((template) => (
            <button
              key={template.id}
              type="button"
              className="templates-list-item"
              aria-current={selection.kind === "custom" && selection.template.id === template.id}
              onClick={() => setSelection({ kind: "custom", template })}
            >
              <span>{template.name}</span>
            </button>
          ))}
          <button
            type="button"
            className="templates-list-item templates-list-add"
            aria-current={selection.kind === "new"}
            onClick={() => setSelection({ kind: "new" })}
          >
            + 나만의 템플릿
          </button>
        </nav>

        <div className="templates-editor">
          {error && (
            <div className="error-banner" role="alert">
              {error}
            </div>
          )}

          <div className="templates-editor-head">
            <div>
              <p className="templates-editor-eyebrow">
                {selection.kind === "new" ? "새 템플릿" : "템플릿 편집"}
              </p>
              <b className="templates-editor-title">{selection.kind === "new" ? "이름 없음" : name}</b>
            </div>
            {isReadOnly && <Badge variant="outline">프리셋 · 읽기 전용</Badge>}
          </div>

          <div className="templates-field">
            <label htmlFor="template-name">이름</label>
            <input
              id="template-name"
              className="templates-input"
              value={name}
              onChange={(e) => {
                setName(e.target.value);
                if (e.target.value.trim()) setNameError(null);
              }}
              disabled={isReadOnly}
              aria-invalid={Boolean(nameError)}
              placeholder="예: 스프린트 회고"
            />
            {nameError && <p className="templates-field-error">{nameError}</p>}
          </div>

          <div className="templates-field">
            <label htmlFor="template-content">내용 (섹션별 줄바꿈)</label>
            <textarea
              id="template-content"
              className="templates-textarea"
              value={content}
              onChange={(e) => setContent(e.target.value)}
              disabled={isReadOnly}
            />
          </div>

          {!isReadOnly && (
            <div className="templates-editor-actions">
              <Button size="sm" onClick={() => void handleSave()} disabled={isSaving}>
                {isSaving ? "저장 중..." : selection.kind === "new" ? "템플릿 만들기" : "저장"}
              </Button>
              <div className="spacer" />
              {selection.kind === "custom" && (
                <Button
                  variant="ghost"
                  size="sm"
                  className="text-destructive hover:bg-destructive-soft hover:text-destructive"
                  onClick={() => void handleDelete()}
                  disabled={isSaving}
                >
                  삭제
                </Button>
              )}
            </div>
          )}

          {isLoading && <p className="templates-empty">템플릿을 불러오는 중...</p>}
        </div>
      </div>
    </section>
  );
}
