import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { errorMessage } from "../formatters";
import type { AppSettings, LlmProvider, OAuthStatus } from "../types";

interface SettingsModalProps {
  open: boolean;
  onClose: () => void;
}

type OAuthProviderId = "codex_oauth" | "claude_oauth";

type OAuthCardState =
  | { kind: "loading" }
  | { kind: "ready"; status: OAuthStatus }
  | { kind: "error" };

interface ProviderCard {
  id: LlmProvider;
  name: string;
  sub: string;
  oauth: OAuthProviderId | null;
}

const PROVIDER_CARDS: ProviderCard[] = [
  {
    id: "litellm",
    name: "LiteLLM 게이트웨이",
    sub: "192.168.1.189:4000 · API 키 방식",
    oauth: null,
  },
  {
    id: "codex_oauth",
    name: "ChatGPT 구독 (Codex OAuth)",
    sub: "~/.codex/auth.json",
    oauth: "codex_oauth",
  },
  {
    id: "claude_oauth",
    name: "Claude 구독 (Claude OAuth)",
    sub: "~/.claude/.credentials.json",
    oauth: "claude_oauth",
  },
];

const OAUTH_PROVIDERS: OAuthProviderId[] = ["codex_oauth", "claude_oauth"];

const RELOGIN_COMMANDS: { label: string; command: string }[] = [
  { label: "ChatGPT 구독", command: "codex login" },
  { label: "Claude 구독", command: "claude login" },
];

const INITIAL_OAUTH_STATE: Record<OAuthProviderId, OAuthCardState> = {
  codex_oauth: { kind: "loading" },
  claude_oauth: { kind: "loading" },
};

function describeStatus(state: OAuthCardState): { tone: string; text: string } {
  if (state.kind === "loading") return { tone: "pending", text: "상태 확인 중..." };
  if (state.kind === "error") return { tone: "unknown", text: "상태 확인 불가" };

  const { logged_in, access_expired, expires_at } = state.status;
  if (!logged_in) return { tone: "off", text: "미로그인" };
  if (access_expired) return { tone: "warn", text: "액세스 토큰 만료 · 사용 시 자동 갱신" };
  if (expires_at === null) return { tone: "ok", text: "만료 시각 확인 불가 · 정상" };

  return { tone: "ok", text: `만료: ${new Date(expires_at).toLocaleString("ko-KR")} · 정상` };
}

function shortAccountId(state: OAuthCardState): string | null {
  if (state.kind !== "ready" || state.status.account_id === null) return null;
  return `${state.status.account_id.slice(0, 8)}…`;
}

export function SettingsModal({ open, onClose }: SettingsModalProps) {
  const [selected, setSelected] = useState<LlmProvider>("litellm");
  const [oauthState, setOauthState] =
    useState<Record<OAuthProviderId, OAuthCardState>>(INITIAL_OAUTH_STATE);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [copiedCommand, setCopiedCommand] = useState<string | null>(null);

  const dialogRef = useRef<HTMLDivElement>(null);
  const copyTimerRef = useRef<number | null>(null);

  useEffect(() => {
    if (!open) return;

    let cancelled = false;
    setLoadError(null);
    setSaveError(null);
    setOauthState(INITIAL_OAUTH_STATE);

    const load = async () => {
      try {
        const settings = await invoke<AppSettings>("get_app_settings");
        if (!cancelled) setSelected(settings.llm_provider);
      } catch (error) {
        if (!cancelled) setLoadError(errorMessage(error));
      }

      await Promise.all(
        OAUTH_PROVIDERS.map(async (provider) => {
          try {
            const status = await invoke<OAuthStatus>("get_oauth_status", { provider });
            if (!cancelled) {
              setOauthState((previous) => ({ ...previous, [provider]: { kind: "ready", status } }));
            }
          } catch {
            if (!cancelled) {
              setOauthState((previous) => ({ ...previous, [provider]: { kind: "error" } }));
            }
          }
        }),
      );
    };

    void load();
    return () => {
      cancelled = true;
    };
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

  useEffect(
    () => () => {
      if (copyTimerRef.current !== null) window.clearTimeout(copyTimerRef.current);
    },
    [],
  );

  if (!open) return null;

  const copyCommand = async (command: string) => {
    try {
      await navigator.clipboard.writeText(command);
      setCopiedCommand(command);
      if (copyTimerRef.current !== null) window.clearTimeout(copyTimerRef.current);
      copyTimerRef.current = window.setTimeout(() => setCopiedCommand(null), 1500);
    } catch (error) {
      setSaveError(errorMessage(error));
    }
  };

  const saveSettings = async () => {
    setIsSaving(true);
    setSaveError(null);
    try {
      await invoke<void>("set_app_settings", { settings: { llm_provider: selected } });
      onClose();
    } catch (error) {
      setSaveError(errorMessage(error));
    } finally {
      setIsSaving(false);
    }
  };

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
            <p className="eyebrow">SETTINGS</p>
            <h2>AI 공급자</h2>
            <p className="settings-subtitle">회의록 생성에 사용할 모델 공급자를 선택하세요.</p>
          </div>
          <button type="button" className="settings-close" aria-label="설정 닫기" onClick={onClose}>
            ✕
          </button>
        </header>

        {loadError && <div className="error-banner">설정을 불러오지 못했습니다: {loadError}</div>}

        <p className="settings-section-label">공급자 선택</p>
        <div className="provider-list" role="radiogroup" aria-label="LLM 공급자">
          {PROVIDER_CARDS.map((card) => {
            const isSelected = card.id === selected;
            const state = card.oauth === null ? null : oauthState[card.oauth];
            const status = state === null ? null : describeStatus(state);
            const accountId = state === null ? null : shortAccountId(state);

            return (
              <button
                key={card.id}
                type="button"
                role="radio"
                aria-checked={isSelected}
                className={`provider-card ${isSelected ? "selected" : ""}`}
                onClick={() => setSelected(card.id)}
                disabled={isSaving}
              >
                <span className="provider-radio" aria-hidden="true" />
                <span className="provider-body">
                  <span className="provider-name">{card.name}</span>
                  <span className="provider-sub">{card.sub}</span>
                  {status && (
                    <span className={`provider-status ${status.tone}`}>
                      <span className="provider-status-text">{status.text}</span>
                      {accountId && <span className="provider-account">{accountId}</span>}
                    </span>
                  )}
                </span>
              </button>
            );
          })}
        </div>

        <div className="settings-guide">
          <p>구독 로그인이 만료되면 터미널에서 아래 명령으로 다시 로그인하세요.</p>
          <div className="settings-guide-commands">
            {RELOGIN_COMMANDS.map(({ label, command }) => (
              <button
                key={command}
                type="button"
                className={`copy-command ${copiedCommand === command ? "copied" : ""}`}
                aria-label={`${label} 재로그인 명령 복사`}
                onClick={() => void copyCommand(command)}
              >
                <code>{command}</code>
                <span className="copy-command-hint">
                  {copiedCommand === command ? "복사됨" : "복사"}
                </span>
              </button>
            ))}
          </div>
        </div>

        {saveError && <div className="error-banner">설정을 저장하지 못했습니다: {saveError}</div>}

        <div className="settings-actions">
          <button type="button" className="button secondary" onClick={onClose} disabled={isSaving}>
            취소
          </button>
          <button
            type="button"
            className="button primary"
            onClick={() => void saveSettings()}
            disabled={isSaving}
          >
            {isSaving ? "저장 중..." : "저장"}
          </button>
        </div>
      </div>
    </div>
  );
}
