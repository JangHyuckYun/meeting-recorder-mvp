import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
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
    models: ["gpt-4o", "gpt-4.1-mini", "gpt-4.1-nano"],
  },
  "00000000-0000-0000-0000-000000000002": {
    name: "Claude 구독 (Claude OAuth)",
    type: "anthropic",
    models: ["claude-sonnet-4-20250514", "claude-sonnet-4", "claude-3.5-haiku"],
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

const STT_ENGINE_OPTIONS: { value: SttEngine; label: string }[] = [
  { value: "self_hosted", label: "자체 모델 서버" },
  { value: "elevenlabs", label: "ElevenLabs (Scribe)" },
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
  const [modelsJson, setModelsJson] = useState('["gpt-4.1-mini"]');
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
          placeholder='["gpt-4o", "gpt-4.1-mini"]'
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

  // OAuth status (for built-in OAuth providers)
  const [oauthStatuses, setOauthStatuses] = useState<Record<string, OAuthStatus | null>>({});

  // Voice / STT state
  const [sttServerUrl, setSttServerUrl] = useState("");
  const [sttEngine, setSttEngine] = useState<SttEngine>("self_hosted");
  const [elevenLabsKeyMasked, setElevenLabsKeyMasked] = useState<string | null>(null);
  const [elevenLabsKeyInput, setElevenLabsKeyInput] = useState("");
  const [isSavingKey, setIsSavingKey] = useState(false);
  const [voiceError, setVoiceError] = useState<string | null>(null);

  // General
  const [error, setError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const dialogRef = useRef<HTMLDivElement>(null);

  // ── Load data on open ────────────────────────────────────────────────

  const loadAppSettings = async () => {
    const appSettings = await invoke<AppSettings>("get_app_settings");
    setSttServerUrl(appSettings.stt_server_url ?? "");
    setSttEngine(appSettings.stt_engine ?? "self_hosted");
    setElevenLabsKeyMasked(appSettings.elevenlabs_api_key_masked ?? null);
  };

  useEffect(() => {
    if (!open) return;
    setShowAddForm(false);
    setError(null);
    setAssignmentError(null);
    setVoiceError(null);
    setElevenLabsKeyInput("");

    const load = async () => {
      try {
        const provs = await invoke<Provider[]>("list_providers");
        setProviders(provs);
        setProviderError(null);
      } catch (e) {
        setProviderError(errorMessage(e));
      }

      try {
        const assigns = await invoke<ModelAssignment[]>("get_model_assignments");
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
          const status = await invoke<OAuthStatus>("get_oauth_status", { provider: pid });
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
      await invoke<string>("add_provider", { input });
      const provs = await invoke<Provider[]>("list_providers");
      setProviders(provs);
      setShowAddForm(false);
    } catch (e) {
      setProviderError(errorMessage(e));
    }
  };

  const handleDeleteProvider = async (id: string) => {
    setProviderError(null);
    try {
      await invoke<void>("delete_provider", { id });
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

  const handleUpdateAssignment = async (purpose: string, providerId: string, modelName: string) => {
    if (!providerId || !modelName) return;
    setAssignmentError(null);
    try {
      const input: ModelAssignmentInput = { purpose, provider_id: providerId, model_name: modelName };
      await invoke<void>("set_model_assignment", { input });
      setAssignments((prev) => ({
        ...prev,
        [purpose]: { purpose: purpose as ModelAssignment["purpose"], provider_id: providerId, model_name: modelName },
      }));
    } catch (e) {
      setAssignmentError(errorMessage(e));
    }
  };

  const handleSaveElevenLabsKey = async () => {
    const apiKey = elevenLabsKeyInput.trim();
    if (!apiKey) return;
    setIsSavingKey(true);
    setVoiceError(null);
    try {
      await invoke<void>("set_elevenlabs_api_key", { apiKey });
      setElevenLabsKeyInput("");
      await loadAppSettings();
    } catch (e) {
      setVoiceError(errorMessage(e));
    } finally {
      setIsSavingKey(false);
    }
  };

  const handleSave = () => {
    setIsSaving(true);
    void (async () => {
      try {
        await invoke<void>("set_app_settings", {
          settings: {
            llm_provider: "codex_oauth" as LlmProvider,
            stt_server_url: sttServerUrl || null,
            stt_engine: sttEngine,
            elevenlabs_api_key_masked: elevenLabsKeyMasked,
          } satisfies AppSettings,
        });
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
  }));

  if (!open) return null;

  // ── Render ───────────────────────────────────────────────────────────

  return (
    <div
      className="settings-overlay"
      onClick={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        className="settings-panel"
        role="dialog"
        aria-modal="true"
        aria-label="설정"
        tabIndex={-1}
        ref={dialogRef}
      >
        <header className="settings-header">
          <div>
            <p className="settings-eyebrow">SETTINGS</p>
            <h2>환경 설정</h2>
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

        <Tabs defaultValue="models" className="contents">
          <div className="settings-tabbar">
            <TabsList>
              <TabsTrigger value="models" data-testid="settings-tab-models">
                모델
              </TabsTrigger>
              <TabsTrigger value="voice" data-testid="settings-tab-voice">
                음성
              </TabsTrigger>
            </TabsList>
          </div>

          {error && <div className="error-banner settings-alert">{error}</div>}

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
                        </div>
                      </div>
                    );
                  })}
                </div>
              </section>
            </TabsContent>

            {/* ── Tab: 음성 ─────────────────────────────────────────── */}
            <TabsContent value="voice" className="settings-tab-panel">
              <section className="settings-section">
                <p className="settings-section-label">전사 엔진</p>
                <p className="settings-subtitle">녹음을 텍스트로 변환할 엔진을 선택하세요.</p>

                {voiceError && <div className="error-banner">{voiceError}</div>}

                <div className="settings-rows">
                  <label className="settings-row">
                    <span className="settings-row-label">엔진</span>
                    <select
                      className="settings-select"
                      data-testid="stt-engine-select"
                      value={sttEngine}
                      onChange={(e) => setSttEngine(e.target.value as SttEngine)}
                    >
                      {STT_ENGINE_OPTIONS.map((o) => (
                        <option key={o.value} value={o.value}>
                          {o.label}
                        </option>
                      ))}
                    </select>
                  </label>

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
