import { useEffect, useRef, useState } from "react";
import { appClient } from "@/platform/appClient";
import { errorMessage } from "../formatters";
import type {
  AppSettings,
  LlmProvider,
  ModelAssignment,
  ModelAssignmentInput,
  OAuthStatus,
  Provider,
  ProviderInput,
  SttEngine,
} from "../types";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";

interface SettingsModalProps {
  open: boolean;
  onClose: () => void;
}

// ── Constants ──────────────────────────────────────────────────────────

const OAUTH_PROVIDER_IDS: string[] = ["codex_oauth", "claude_oauth"];

const BUILTIN_PROVIDER_MAP: Record<string, { name: string; type: string; models: string[] }> = {
  "00000000-0000-0000-0000-000000000001": {
    name: "ChatGPT 구독 (Codex OAuth)",
    type: "openai",
    models: ["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna", "gpt-5.5", "gpt-5.4", "gpt-5.4-mini", "gpt-5.4-nano", "gpt-5.1", "gpt-5.1-codex", "gpt-5.1-codex-max"],
  },
  "00000000-0000-0000-0000-000000000002": {
    name: "Claude 구독 (Claude OAuth)",
    type: "anthropic",
    models: ["claude-fable-5", "claude-opus-5", "claude-opus-4-8", "claude-opus-4-7", "claude-opus-4-6", "claude-sonnet-5", "claude-sonnet-4-6", "claude-haiku-4-5"],
  },
};

const MODEL_PURPOSES: { purpose: string; label: string; desc: string }[] = [
  { purpose: "minutes_generation", label: "회의록 생성", desc: "전사로부터 요약·결정·할 일을 생성합니다." },
  { purpose: "minutes_edit", label: "회의록 항목 수정", desc: "기존 항목을 지시에 따라 수정합니다." },
];

const PROVIDER_TYPE_LABELS: Record<string, string> = {
  openai: "OpenAI",
  anthropic: "Anthropic",
  openai_compatible: "OpenAI 호환",
};

const PROVIDER_TYPE_OPTIONS = [
  { value: "openai", label: "OpenAI" },
  { value: "anthropic", label: "Anthropic" },
  { value: "openai_compatible", label: "OpenAI 호환 (vLLM, Ollama 등)" },
];

const STT_ENGINE_OPTIONS: {
  value: SttEngine;
  label: string;
  role: string;
  desc: string;
  tags: string[];
}[] = [
  {
    value: "elevenlabs",
    label: "ElevenLabs Scribe",
    role: "기본",
    desc: "v2 Realtime 라이브 캡션 + v2 배치 화자분리",
    tags: ["클라우드", "다국어", "keyterm prompting"],
  },
  {
    value: "self_hosted",
    label: "자체 모델 서버",
    role: "폴백",
    desc: "자체 GPU 스택 · 로컬 서버에 연결",
    tags: ["오프라인", "보안망", "원가통제"],
  },
];

// ── Helpers ────────────────────────────────────────────────────────────

function describeOAuthStatus(status: OAuthStatus | null): { tone: string; text: string } {
  if (!status) return { tone: "pending", text: "상태 확인 중..." };
  if (!status.logged_in) return { tone: "off", text: "미로그인" };
  if (status.access_expired)
    return { tone: "warn", text: "액세스 토큰 만료 · 사용 시 자동 갱신" };
  if (!status.expires_at) return { tone: "ok", text: "만료 시각 확인 불가 · 정상" };
  return {
    tone: "ok",
    text: `만료: ${new Date(status.expires_at).toLocaleString("ko-KR")} · 정상`,
  };
}

function CloseIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8">
      <path d="M6 6l12 12M18 6L6 18" strokeLinecap="round" />
    </svg>
  );
}

// ── Sub-components ─────────────────────────────────────────────────────

function AddProviderForm({
  onSave,
  onCancel,
}: {
  onSave: (input: ProviderInput) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState("");
  const [providerType, setProviderType] = useState("openai");
  const [baseUrl, setBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [modelsJson, setModelsJson] = useState('["gpt-5.6-terra"]');
  const [saving, setSaving] = useState(false);

  const handleSave = async () => {
    if (!name.trim()) return;
    setSaving(true);
    try {
      onSave({
        name: name.trim(),
        provider_type: providerType,
        base_url: baseUrl.trim(),
        api_key: apiKey,
        models_json: modelsJson,
      });
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="add-provider-form">
      <h4>새 공급자 추가</h4>
      <label className="settings-row">
        <span className="settings-row-label">이름</span>
        <input
          className="settings-input"
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="예: My OpenAI, 로컬 vLLM"
        />
      </label>
      <label className="settings-row">
        <span className="settings-row-label">유형</span>
        <select
          className="settings-select"
          value={providerType}
          onChange={(e) => setProviderType(e.target.value)}
        >
          {PROVIDER_TYPE_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
      </label>
      {providerType !== "openai" && (
        <label className="settings-row">
          <span className="settings-row-label">Base URL</span>
          <input
            className="settings-input"
            type="text"
            data-numeric
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            placeholder="예: http://localhost:8000/v1"
          />
        </label>
      )}
      <label className="settings-row">
        <span className="settings-row-label">API 키</span>
        <input
          className="settings-input"
          type="password"
          data-numeric
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          placeholder="sk-…"
        />
      </label>
      <label className="settings-row">
        <span className="settings-row-label">
          모델 목록
          <span>JSON 배열</span>
        </span>
        <input
          className="settings-input"
          type="text"
          data-numeric
          value={modelsJson}
          onChange={(e) => setModelsJson(e.target.value)}
          placeholder='["gpt-5.6-terra"]'
        />
      </label>
      <div className="add-provider-actions">
        <Button variant="ghost" size="sm" onClick={onCancel} disabled={saving}>
          취소
        </Button>
        <Button size="sm" onClick={() => void handleSave()} disabled={saving || !name.trim()}>
          {saving ? "추가 중..." : "추가"}
        </Button>
      </div>
    </div>
  );
}

// ── Main component ─────────────────────────────────────────────────────

export function SettingsModal({ open, onClose }: SettingsModalProps) {
  // Provider registry state
  const [providers, setProviders] = useState<Provider[]>([]);
  const [showAddForm, setShowAddForm] = useState(false);
  const [providerError, setProviderError] = useState<string | null>(null);

  // Model assignment state
  const [assignments, setAssignments] = useState<Record<string, ModelAssignment>>({});
  const [assignmentError, setAssignmentError] = useState<string | null>(null);
  const [loadingModels, setLoadingModels] = useState<string | null>(null);

  // OAuth status (for built-in OAuth providers)
  const [oauthStatuses, setOauthStatuses] = useState<Record<string, OAuthStatus | null>>({});

  // Voice / STT state
  const [sttServerUrl, setSttServerUrl] = useState("");
  const [sttEngine, setSttEngine] = useState<SttEngine>("self_hosted");
  const [speakers, setSpeakers] = useState<number | null>(null);
  const [elevenLabsKeyMasked, setElevenLabsKeyMasked] = useState<string | null>(null);
  const [elevenLabsKeyInput, setElevenLabsKeyInput] = useState("");
  const [isSavingKey, setIsSavingKey] = useState(false);
  const [voiceError, setVoiceError] = useState<string | null>(null);

  // Glossary / keyterms
  const [glossaryTerms, setGlossaryTerms] = useState<string[]>([]);
  const [glossaryDraft, setGlossaryDraft] = useState("");
  const [glossaryError, setGlossaryError] = useState<string | null>(null);

  // General
  const [error, setError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const dialogRef = useRef<HTMLDivElement>(null);

  // ── Load data on open ────────────────────────────────────────────────

  const loadAppSettings = async () => {
    const appSettings = await appClient.getAppSettings();
    setSttServerUrl(appSettings.stt_server_url ?? "");
    setSttEngine(appSettings.stt_engine ?? "self_hosted");
    setSpeakers(appSettings.speakers ?? null);
    setElevenLabsKeyMasked(appSettings.elevenlabs_api_key_masked ?? null);
  };

  useEffect(() => {
    if (!open) return;
    setShowAddForm(false);
    setError(null);
    setAssignmentError(null);
    setVoiceError(null);
    setElevenLabsKeyInput("");
    setGlossaryError(null);

    appClient
      .getGlossary()
      .then(setGlossaryTerms)
      .catch((e) => setGlossaryError(errorMessage(e)));

    const load = async () => {
      try {
        const provs = await appClient.listProviders();
        setProviders(provs);
        setProviderError(null);
      } catch (e) {
        setProviderError(errorMessage(e));
      }

      try {
        const assigns = await appClient.getModelAssignments();
        const assignMap: Record<string, ModelAssignment> = {};
        for (const a of assigns) {
          assignMap[a.purpose] = a;
        }
        setAssignments(assignMap);
        setAssignmentError(null);
      } catch (e) {
        setAssignmentError(errorMessage(e));
      }

      // Load current settings (STT server URL, engine, masked key)
      try {
        await loadAppSettings();
      } catch { /* ignore */ }

      // Load OAuth statuses for built-in providers
      for (const pid of OAUTH_PROVIDER_IDS) {
        try {
          const status = await appClient.getOAuthStatus(pid);
          setOauthStatuses((prev) => ({ ...prev, [pid]: status }));
        } catch {
          setOauthStatuses((prev) => ({ ...prev, [pid]: null }));
        }
      }
    };

    void load();
  }, [open]);

  useEffect(() => {
    if (!open) return;
    dialogRef.current?.focus();
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [open, onClose]);

  // ── Handlers ─────────────────────────────────────────────────────────

  const handleAddProvider = async (input: ProviderInput) => {
    setProviderError(null);
    try {
      await appClient.addProvider(input);
      const provs = await appClient.listProviders();
      setProviders(provs);
      setShowAddForm(false);
    } catch (e) {
      setProviderError(errorMessage(e));
    }
  };

  const handleDeleteProvider = async (id: string) => {
    setProviderError(null);
    try {
      await appClient.deleteProvider(id);
      setProviders((prev) => prev.filter((p) => p.id !== id));
      // Also clear any assignment referencing this provider
      setAssignments((prev) => {
        const next = { ...prev };
        for (const [p, a] of Object.entries(next)) {
          if (a.provider_id === id) delete next[p];
        }
        return next;
      });
    } catch (e) {
      setProviderError(errorMessage(e));
    }
  };

  const handleUpdateAssignment = async (purpose: string, providerId: string, modelName: string, reasoning_effort: string | null = null, fast = false) => {
    if (!providerId || !modelName) return;
    setAssignmentError(null);
    try {
      const input: ModelAssignmentInput = { purpose, provider_id: providerId, model_name: modelName, reasoning_effort, fast };
      await appClient.setModelAssignment(input);
      setAssignments((prev) => ({
        ...prev,
        [purpose]: { purpose: purpose as ModelAssignment["purpose"], provider_id: providerId, model_name: modelName, reasoning_effort, fast },
      }));
    } catch (e) {
      setAssignmentError(errorMessage(e));
    }
  };

  const handleLoadModels = async (providerId: string) => {
    setLoadingModels(providerId);
    try {
      const models = await appClient.listRemoteModels(providerId);
      setProviders((prev) => prev.map((p) => p.id === providerId ? { ...p, models } : p));
      const provider = providers.find((p) => p.id === providerId);
      if (provider && !provider.is_builtin) await appClient.updateProvider({ id: provider.id, name: provider.name, provider_type: provider.provider_type, base_url: provider.base_url, api_key: "", models_json: JSON.stringify(models) });
    } catch (e) { setProviderError(errorMessage(e)); }
    finally { setLoadingModels(null); }
  };

  const handleSaveElevenLabsKey = async () => {
    const apiKey = elevenLabsKeyInput.trim();
    if (!apiKey) return;
    setIsSavingKey(true);
    setVoiceError(null);
    try {
      await appClient.setElevenLabsApiKey(apiKey);
      setElevenLabsKeyInput("");
      await loadAppSettings();
    } catch (e) {
      setVoiceError(errorMessage(e));
    } finally {
      setIsSavingKey(false);
    }
  };

  const persistGlossary = async (terms: string[]) => {
    setGlossaryError(null);
    try {
      await appClient.setGlossary(terms);
      setGlossaryTerms(terms);
    } catch (e) {
      setGlossaryError(errorMessage(e));
    }
  };

  const handleAddGlossaryTerm = () => {
    const term = glossaryDraft.trim();
    if (!term || glossaryTerms.includes(term)) {
      setGlossaryDraft("");
      return;
    }
    setGlossaryDraft("");
    void persistGlossary([...glossaryTerms, term]);
  };

  const handleRemoveGlossaryTerm = (term: string) => {
    void persistGlossary(glossaryTerms.filter((t) => t !== term));
  };

  const handleSave = () => {
    setIsSaving(true);
    void (async () => {
      try {
        await appClient.setAppSettings({
          llm_provider: "codex_oauth" as LlmProvider,
          stt_server_url: sttServerUrl || null,
          stt_engine: sttEngine,
          speakers,
          elevenlabs_api_key_masked: elevenLabsKeyMasked,
        } satisfies AppSettings);
        onClose();
      } catch (e) {
        setError(errorMessage(e));
      } finally {
        setIsSaving(false);
      }
    })();
  };

  // ── Derived state ────────────────────────────────────────────────────

  const providerOptions = providers.map((p) => ({
    value: p.id,
    label: p.is_builtin
      ? `${BUILTIN_PROVIDER_MAP[p.id]?.name ?? p.name} (기본)`
      : p.name,
    models: p.models.length > 0 ? p.models : (BUILTIN_PROVIDER_MAP[p.id]?.models ?? []),
    provider_type: p.provider_type,
  }));

  if (!open) return null;

  // ── Render ───────────────────────────────────────────────────────────

  return (
    <div className="settings-overlay">
      <div
        className="settings-panel"
        role="region"
        aria-label="설정"
        tabIndex={-1}
        ref={dialogRef}
      >
        <header className="settings-header">
          <div>
            <p className="settings-eyebrow">SETTINGS</p>
            <h2>설정</h2>
          </div>
          <Button
            variant="ghost"
            size="icon"
            className="size-8"
            aria-label="설정 닫기"
            onClick={onClose}
          >
            <CloseIcon />
          </Button>
        </header>

        {error && <div className="error-banner settings-alert">{error}</div>}

        <Tabs defaultValue="voice" className="settings-shell">
          <TabsList className="settings-rail flex-col items-stretch justify-start gap-1 rounded-none bg-transparent p-2 h-auto w-full">
            <TabsTrigger
              value="voice"
              data-testid="settings-tab-voice"
              className="settings-rail-trigger"
            >
              STT 엔진
            </TabsTrigger>
            <TabsTrigger
              value="glossary"
              data-testid="settings-tab-glossary"
              className="settings-rail-trigger"
            >
              단어장
            </TabsTrigger>
            <TabsTrigger
              value="models"
              data-testid="settings-tab-models"
              className="settings-rail-trigger"
            >
              회의록 모델
            </TabsTrigger>
          </TabsList>

          <div className="settings-body ds-scroll">
            {/* ── Tab: 모델 ─────────────────────────────────────────── */}
            <TabsContent value="models" className="settings-tab-panel">
              {/* ── Panel A: Provider Management ───────────────────── */}
              <section className="settings-section">
                <div className="settings-section-header">
                  <div>
                    <p className="settings-section-label">공급자 관리</p>
                    <p className="settings-subtitle">
                      회의록 생성에 사용할 LLM 공급자와 자격 증명을 관리합니다.
                    </p>
                  </div>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => setShowAddForm(!showAddForm)}
                    disabled={showAddForm}
                  >
                    + 공급자 추가
                  </Button>
                </div>

                {providerError && <div className="error-banner">{providerError}</div>}

                {showAddForm && (
                  <AddProviderForm
                    onSave={handleAddProvider}
                    onCancel={() => setShowAddForm(false)}
                  />
                )}

                {providers.length === 0 && !showAddForm && (
                  <p className="empty-small">
                    등록된 공급자가 없습니다. 위 버튼을 눌러 추가하세요.
                  </p>
                )}

                {providers.length > 0 && (
                  <div className="settings-rows">
                    {providers.map((provider) => {
                      const builtinInfo = BUILTIN_PROVIDER_MAP[provider.id];
                      // Map builtin providers to their OAuth keys correctly
                      let oauthProviderId: string | null = null;
                      if (provider.id === "00000000-0000-0000-0000-000000000001") oauthProviderId = "codex_oauth";
                      else if (provider.id === "00000000-0000-0000-0000-000000000002") oauthProviderId = "claude_oauth";
                      const oauthStatus = oauthProviderId ? oauthStatuses[oauthProviderId] : null;
                      const st = oauthStatus ? describeOAuthStatus(oauthStatus) : null;
                      const models = provider.models.length > 0 ? provider.models : (builtinInfo?.models ?? []);

                      return (
                        <div key={provider.id} className="provider-row">
                          <div className="entry-main">
                            <div className="entry-heading">
                              <span className="entry-name">
                                {builtinInfo?.name ?? provider.name}
                              </span>
                              <span className="entry-type">
                                {PROVIDER_TYPE_LABELS[provider.provider_type] ?? provider.provider_type}
                                {provider.is_builtin && " · 기본"}
                              </span>
                            </div>
                            {!provider.is_builtin && provider.base_url && (
                              <div className="entry-detail" data-numeric>
                                {provider.base_url}
                              </div>
                            )}
                            {provider.api_key_masked && (
                              <div className="entry-detail">
                                키: <code data-numeric>{provider.api_key_masked}</code>
                              </div>
                            )}
                            {models.length > 0 && (
                              <div className="entry-models" data-numeric>
                                {models.map((m) => (
                                  <span key={m}>{m}</span>
                                ))}
                              </div>
                            )}
                            {st && <div className={`provider-status ${st.tone}`}>{st.text}</div>}
                          </div>
                          {!provider.is_builtin && (
                            <div className="entry-actions">
                              <Button
                                variant="ghost"
                                size="sm"
                                className="text-destructive hover:bg-destructive-soft hover:text-destructive"
                                onClick={() => void handleDeleteProvider(provider.id)}
                                title="삭제"
                              >
                                삭제
                              </Button>
                            </div>
                          )}
                          <Button variant="ghost" size="sm" onClick={() => void handleLoadModels(provider.id)} disabled={loadingModels === provider.id}>
                            {loadingModels === provider.id ? "불러오는 중..." : "모델 불러오기"}
                          </Button>
                        </div>
                      );
                    })}
                  </div>
                )}
              </section>

              {/* ── Panel B: Model Assignment ──────────────────────── */}
              <section className="settings-section">
                <p className="settings-section-label">모델 할당</p>
                <p className="settings-subtitle">각 작업에 사용할 공급자와 모델을 선택하세요.</p>

                {assignmentError && <div className="error-banner">{assignmentError}</div>}

                <div className="settings-rows">
                  {MODEL_PURPOSES.map(({ purpose, label, desc }) => {
                    const current = assignments[purpose];
                    const selectedProviderId = current?.provider_id ?? "";
                    const selectedProvider = providerOptions.find((o) => o.value === selectedProviderId);
                    const availableModels = selectedProvider?.models ?? [];

                    return (
                      <div key={purpose} className="settings-row">
                        <div className="settings-row-label">
                          {label}
                          <span>{desc}</span>
                        </div>
                        <div className="settings-controls">
                          <select
                            className="settings-select"
                            aria-label={`${label} 공급자`}
                            value={selectedProviderId}
                            onChange={(e) => {
                              const pid = e.target.value;
                              const provider = providerOptions.find((o) => o.value === pid);
                              const firstModel = provider?.models[0] ?? "";
                              void handleUpdateAssignment(purpose, pid, firstModel);
                            }}
                          >
                            <option value="">-- 공급자 선택 --</option>
                            {providerOptions.map((o) => (
                              <option key={o.value} value={o.value}>
                                {o.label}
                              </option>
                            ))}
                          </select>
                          {selectedProviderId && (
                            <select
                              className="settings-select"
                              aria-label={`${label} 모델`}
                              data-numeric
                              value={current?.model_name ?? ""}
                              onChange={(e) =>
                                void handleUpdateAssignment(purpose, selectedProviderId, e.target.value)
                              }
                            >
                              <option value="">-- 모델 선택 --</option>
                              {availableModels.map((m) => (
                                <option key={m} value={m}>
                                  {m}
                                </option>
                              ))}
                            </select>
                          )}
                          {selectedProviderId && (() => {
                            const anthropic = selectedProvider?.provider_type === "anthropic";
                            const model = current?.model_name ?? "";
                            const maxAllowed = model.startsWith("gpt-5.6");
                            const efforts = anthropic ? ["", "low", "medium", "high", "xhigh", "max"] : ["", "none", "low", "medium", "high", ...(maxAllowed ? ["xhigh", "max"] : [])];
                            const fastAllowed = selectedProvider?.provider_type === "openai" || selectedProvider?.provider_type === "openai_compatible" || (anthropic && (model.startsWith("claude-opus-5") || model === "claude-opus-4-8"));
                            return <>
                              <select className="settings-select" aria-label={`${label} Reasoning`} value={current?.reasoning_effort ?? ""} onChange={(e) => void handleUpdateAssignment(purpose, selectedProviderId, model, e.target.value || null, current?.fast ?? false)}>
                                {efforts.map((e) => <option key={e} value={e}>{e || (anthropic ? "없음" : "none")}</option>)}
                              </select>
                              <label><input type="checkbox" checked={fastAllowed && (current?.fast ?? false)} disabled={!fastAllowed} onChange={(e) => void handleUpdateAssignment(purpose, selectedProviderId, model, current?.reasoning_effort ?? null, e.target.checked)} /> Fast</label>
                            </>;
                          })()}
                        </div>
                      </div>
                    );
                  })}
                </div>
              </section>
            </TabsContent>

            {/* ── Tab: 단어장 ───────────────────────────────────────── */}
            <TabsContent value="glossary" className="settings-tab-panel">
              <section className="settings-section">
                <p className="settings-section-label">개인 단어장</p>
                <p className="settings-subtitle">
                  고유명사·전문 용어를 등록하면 음성 인식 정확도가 올라갑니다.
                </p>

                {glossaryError && <div className="error-banner">{glossaryError}</div>}

                <div className="glossary-chips">
                  {glossaryTerms.map((term) => (
                    <span key={term} className="glossary-chip">
                      {term}
                      <button
                        type="button"
                        aria-label={`${term} 삭제`}
                        onClick={() => handleRemoveGlossaryTerm(term)}
                      >
                        ×
                      </button>
                    </span>
                  ))}
                  <input
                    className="glossary-add-input"
                    value={glossaryDraft}
                    onChange={(e) => setGlossaryDraft(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        e.preventDefault();
                        handleAddGlossaryTerm();
                      }
                    }}
                    onBlur={handleAddGlossaryTerm}
                    placeholder="+ 단어 추가"
                    aria-label="단어 추가"
                  />
                </div>
              </section>
            </TabsContent>

            {/* ── Tab: 음성 ─────────────────────────────────────────── */}
            <TabsContent value="voice" className="settings-tab-panel">
              <section className="settings-section">
                <p className="settings-section-label">전사 엔진</p>
                <p className="settings-subtitle">녹음을 텍스트로 변환할 엔진을 선택하세요.</p>

                {voiceError && <div className="error-banner">{voiceError}</div>}

                <div className="settings-rows" data-testid="stt-engine-select">
                  {STT_ENGINE_OPTIONS.map((o) => (
                    <button
                      key={o.value}
                      type="button"
                      className="engine-option"
                      aria-pressed={sttEngine === o.value}
                      onClick={() => setSttEngine(o.value)}
                    >
                      <span className="engine-option-dot" aria-hidden="true">
                        {sttEngine === o.value ? "◉" : "○"}
                      </span>
                      <span className="engine-option-body">
                        <span className="engine-option-name">
                          {o.label} <Badge variant={o.role === "기본" ? "primary" : "neutral"}>{o.role}</Badge>
                        </span>
                        <span className="engine-option-desc">{o.desc}</span>
                        <span className="engine-option-tags">
                          {o.tags.map((tag) => (
                            <Badge key={tag} variant="outline" size="sm">
                              {tag}
                            </Badge>
                          ))}
                        </span>
                      </span>
                    </button>
                  ))}

                  <label className="settings-row">
                    <span className="settings-row-label">
                      서버 URL
                      <span>자체 모델 서버의 WebSocket 주소</span>
                    </span>
                    <input
                      className="settings-input"
                      type="text"
                      data-numeric
                      data-testid="stt-server-url-input"
                      value={sttServerUrl}
                      onChange={(e) => setSttServerUrl(e.target.value)}
                      placeholder="ws://192.168.1.189:9090"
                    />
                  </label>
                  <label className="settings-row">
                    <span className="settings-row-label">화자 수</span>
                    <select
                      className="settings-select"
                      value={speakers ?? "auto"}
                      onChange={(e) =>
                        setSpeakers(e.target.value === "auto" ? null : Number(e.target.value))
                      }
                    >
                      <option value="auto">자동</option>
                      {Array.from({ length: 9 }, (_, index) => index + 2).map((count) => (
                        <option key={count} value={count}>
                          {count}명
                        </option>
                      ))}
                    </select>
                  </label>
                </div>

                <p className="settings-hint">
                  {sttEngine === "elevenlabs"
                    ? "ElevenLabs Scribe를 사용하려면 아래에 API 키를 등록해야 합니다."
                    : "자체 모델 서버를 사용합니다. 위 서버 URL이 적용됩니다."}
                </p>
              </section>

              <section className="settings-section">
                <p className="settings-section-label">ElevenLabs API 키</p>
                <p className="settings-subtitle">
                  키는 로컬에 저장되며 등록 후에는 마스킹된 값만 표시됩니다.
                </p>

                <div className="settings-rows">
                  <div className="settings-row">
                    <label className="settings-row-label" htmlFor="elevenlabs-api-key">
                      API 키
                    </label>
                    <div className="settings-controls">
                      <input
                        id="elevenlabs-api-key"
                        className="settings-input"
                        type="password"
                        data-numeric
                        data-testid="elevenlabs-api-key-input"
                        value={elevenLabsKeyInput}
                        onChange={(e) => setElevenLabsKeyInput(e.target.value)}
                        placeholder={elevenLabsKeyMasked ?? "sk_…"}
                      />
                      <Button
                        size="sm"
                        onClick={() => void handleSaveElevenLabsKey()}
                        disabled={isSavingKey || !elevenLabsKeyInput.trim()}
                      >
                        {isSavingKey ? "저장 중..." : "저장"}
                      </Button>
                    </div>
                  </div>
                </div>

                <p className="settings-hint">
                  {elevenLabsKeyMasked ? (
                    <>
                      등록된 키: <code data-numeric>{elevenLabsKeyMasked}</code>
                    </>
                  ) : (
                    "등록된 키가 없습니다."
                  )}
                </p>
              </section>
            </TabsContent>
          </div>
        </Tabs>

        {/* ── Save / Cancel ──────────────────────────────────────────── */}
        <div className="settings-footer">
          <Button variant="ghost" size="sm" onClick={onClose} disabled={isSaving}>
            취소
          </Button>
          <Button size="sm" onClick={() => void handleSave()} disabled={isSaving}>
            {isSaving ? "저장 중..." : "저장"}
          </Button>
        </div>
      </div>
    </div>
  );
}
