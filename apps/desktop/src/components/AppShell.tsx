import { useState, type ReactNode } from "react";
import { cn } from "@/lib/utils";
import { SettingsModal } from "./SettingsModal";
import { ErrorBoundary } from "./ErrorBoundary";

type AppView = "live" | "history" | "import";

interface AppShellProps {
  activeView: AppView;
  children: ReactNode;
  onNavigate: (view: AppView) => void;
}

function MicIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <rect x="8" y="3" width="8" height="13" rx="4" />
      <path d="M5 11a7 7 0 0 0 14 0M12 18v3M9 21h6" />
    </svg>
  );
}

function HistoryIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M3 12a9 9 0 1 0 3-6.7L3 8" />
      <path d="M3 3v5h5M12 7v5l3 2" />
    </svg>
  );
}

function ImportIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M12 3v13M12 16l-4-4M12 16l4-4" />
      <path d="M4 17v2a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2" />
    </svg>
  );
}

const NAV_ITEMS: { view: AppView; label: string; title: string; Icon: () => ReactNode }[] = [
  { view: "live", label: "실시간", title: "실시간 녹음", Icon: MicIcon },
  { view: "history", label: "히스토리", title: "녹음 히스토리", Icon: HistoryIcon },
  { view: "import", label: "가져오기", title: "가져오기", Icon: ImportIcon },
];

export function AppShell({ activeView, children, onNavigate }: AppShellProps) {
  const [settingsOpen, setSettingsOpen] = useState(false);

  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="주요 메뉴">
        <div className="brand" aria-label="Minute">
          M
        </div>
        <nav className="sidebar-nav">
          {NAV_ITEMS.map(({ view, label, title, Icon }) => (
            <button
              key={view}
              className={cn("nav-button", activeView === view && "active")}
              type="button"
              title={title}
              aria-label={title}
              aria-current={activeView === view ? "page" : undefined}
              onClick={() => onNavigate(view)}
            >
              <Icon />
              <span>{label}</span>
            </button>
          ))}
        </nav>
        <div className="sidebar-footer">
          <button
            type="button"
            className="profile-dot"
            title="설정 열기"
            aria-label="설정 열기"
            onClick={() => setSettingsOpen(true)}
          >
            윤
          </button>
        </div>
      </aside>
      <main className="workspace">{children}</main>
      <ErrorBoundary>
        <SettingsModal open={settingsOpen} onClose={() => setSettingsOpen(false)} />
      </ErrorBoundary>
    </div>
  );
}
