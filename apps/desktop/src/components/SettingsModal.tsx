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
} from "../types";

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
      <label>
        이름
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="예: My OpenAI, 로컬 vLLM"
        />
      </label>
      <label>
        유형
        <select value={providerType} onChange={(e) => setProviderType(e.target.value)}>
          {PROVIDER_TYPE_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
      </label>
      {providerType !== "openai" && (
        <label>
          Base URL
          <input
            type="text"
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            placeholder="예: http://localhost:8000/v1"
          />
        </label>
      )}
      <label>
        API 키
        <input
          type="password"
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          placeholder="sk-…"
        />
      </label>
      <label>
        모델 목록 (JSON 배열)
        <input
          type="text"
          value={modelsJson}
          onChange={(e) => setModelsJson(e.target.value)}
          placeholder='["gpt-4o", "gpt-4.1-mini"]'
        />
      </label>
      <div className="add-provider-actions">
        <button type="button" className="button secondary" onClick={onCancel} disabled={saving}>
          취소
        </button>
        <button
          type="button"
          className="button primary"
          onClick={() => void handleSave()}
          disabled={saving || !name.trim()}
        >
          {saving ? "추가 중..." : "추가"}
        </button>
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

  // STT server URL
  const [sttServerUrl, setSttServerUrl] = useState("");

  // General
  const [error, setError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const dialogRef = useRef<HTMLDivElement>(null);

  // ── Load data on open ────────────────────────────────────────────────

  useEffect(() => {
    if (!open) return;
    setShowAddForm(false);
    setError(null);
    setAssignmentError(null);

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

      // Load current settings (STT server URL, etc.)
      try {
        const appSettings = await invoke<AppSettings>("get_app_settings");
        setSttServerUrl(appSettings.stt_server_url ?? "");
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

  const handleSave = () => {
    setIsSaving(true);
    void (async () => {
      try {
        await invoke<void>("set_app_settings", {
          settings: {
            llm_provider: "codex_oauth" as LlmProvider,
            stt_server_url: sttServerUrl || null,
          },
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
        className="settings-panel settings-panel-wide"
        role="dialog"
        aria-modal="true"
        aria-label="설정"
        tabIndex={-1}
        ref={dialogRef}
      >
        <header className="settings-header">
          <div>
            <p className="eyebrow">SETTINGS</p>
            <h2>AI 공급자</h2>
            <p className="settings-subtitle">LLM 공급자를 등록하고 용도별 모델을 할당하세요.</p>
          </div>
          <button type="button" className="settings-close" aria-label="설정 닫기" onClick={onClose}>
            ✕
          </button>
        </header>

        {error && <div className="error-banner">{error}</div>}

        {/* ── Panel A: Provider Management ─────────────────────────── */}
        <div className="settings-section">
          <div className="settings-section-header">
            <p className="settings-section-label">공급자 관리</p>
            <button
              type="button"
              className="button secondary add-provider-btn"
              onClick={() => setShowAddForm(!showAddForm)}
              disabled={showAddForm}
            >
              + 공급자 추가
            </button>
          </div>

          {providerError && <div className="error-banner">{providerError}</div>}

          {showAddForm && (
            <AddProviderForm
              onSave={handleAddProvider}
              onCancel={() => setShowAddForm(false)}
            />
          )}

          {providers.length === 0 && !showAddForm && (
            <div className="empty-small">등록된 공급자가 없습니다. 위 버튼을 눌러 추가하세요.</div>
          )}

          <div className="provider-mgmt-list">
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
                <div
                  key={provider.id}
                  className={`provider-mgmt-entry ${provider.is_builtin ? "builtin" : ""}`}
                >
                  <div className="entry-main">
                    <div className="entry-heading">
                      <span className="entry-name">
                        {builtinInfo?.name ?? provider.name}
                      </span>
                      <span className={`type-badge type-${provider.provider_type}`}>
                        {PROVIDER_TYPE_LABELS[provider.provider_type] ?? provider.provider_type}
                      </span>
                      {provider.is_builtin && <span className="builtin-badge">기본</span>}
                    </div>
                    {!provider.is_builtin && provider.base_url && (
                      <div className="entry-detail">{provider.base_url}</div>
                    )}
                    {provider.api_key_masked && (
                      <div className="entry-detail">
                        키: <code>{provider.api_key_masked}</code>
                      </div>
                    )}
                    {models.length > 0 && (
                      <div className="entry-models">
                        {models.map((m) => (
                          <span key={m} className="model-chip">
                            {m}
                          </span>
                        ))}
                      </div>
                    )}
                    {st && (
                      <div className={`provider-status ${st.tone}`}>
                        {st.text}
                      </div>
                    )}
                  </div>
                  <div className="entry-actions">
                    {!provider.is_builtin && (
                      <button
                        type="button"
                        className="button secondary entry-delete"
                        onClick={() => void handleDeleteProvider(provider.id)}
                        title="삭제"
                      >
                        삭제
                      </button>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        </div>

        {/* ── Panel B: Model Assignment ───────────────────────────── */}
        <div className="settings-section">
          <p className="settings-section-label">모델 할당</p>
          <p className="settings-subtitle">각 작업에 사용할 공급자와 모델을 선택하세요.</p>

          {assignmentError && <div className="error-banner">{assignmentError}</div>}

          {MODEL_PURPOSES.map(({ purpose, label, desc }) => {
            const current = assignments[purpose];
            const selectedProviderId = current?.provider_id ?? "";
            const selectedProvider = providerOptions.find((o) => o.value === selectedProviderId);
            const availableModels = selectedProvider?.models ?? [];

            return (
              <div key={purpose} className="model-assign-row">
                <div className="assign-label">
                  <strong>{label}</strong>
                  <span>{desc}</span>
                </div>
                <div className="assign-controls">
                  <select
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

        {/* ── Panel C: STT Server ──────────────────────────────────── */}
        <div className="settings-section">
          <p className="settings-section-label">전사(STT) 서버</p>
          <p className="settings-subtitle">음성 인식 서버의 WebSocket 주소를 입력하세요.</p>
          <div className="provider-mgmt-entry">
            <div className="entry-main">
              <div className="entry-detail">
                <label style={{ display: "flex", flexDirection: "column", gap: 6, fontSize: 11, fontWeight: 700 }}>
                  서버 URL
                  <input
                    type="text"
                    value={sttServerUrl}
                    onChange={(e) => setSttServerUrl(e.target.value)}
                    placeholder="ws://192.168.1.189:9090"
                    style={{ width: "100%", height: 38, padding: "0 10px", fontSize: 12, borderRadius: 9, border: "1px solid hsl(var(--border))", background: "hsl(var(--surface))" }}
                  />
                </label>
              </div>
            </div>
          </div>
        </div>

        {/* ── Save / Cancel ────────────────────────────────────────── */}
        <div className="settings-actions">
          <button type="button" className="button secondary" onClick={onClose} disabled={isSaving}>
            취소
          </button>
          <button
            type="button"
            className="button primary"
            onClick={() => void handleSave()}
            disabled={isSaving}
          >
            {isSaving ? "저장 중..." : "저장"}
          </button>
        </div>
      </div>
    </div>
  );
}