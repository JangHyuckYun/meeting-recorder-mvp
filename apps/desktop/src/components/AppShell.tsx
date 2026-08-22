import type { ReactNode } from "react";

type AppView = "live" | "history";

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

export function AppShell({ activeView, children, onNavigate }: AppShellProps) {
  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="주요 메뉴">
        <div className="brand" aria-label="Minute">
          M
        </div>
        <nav className="sidebar-nav">
          <button
            className={`nav-button ${activeView === "live" ? "active" : ""}`}
            type="button"
            aria-label="실시간 녹음"
            aria-current={activeView === "live" ? "page" : undefined}
            onClick={() => onNavigate("live")}
          >
            <MicIcon />
            <span>실시간</span>
          </button>
          <button
            className={`nav-button ${activeView === "history" ? "active" : ""}`}
            type="button"
            aria-label="녹음 히스토리"
            aria-current={activeView === "history" ? "page" : undefined}
            onClick={() => onNavigate("history")}
          >
            <HistoryIcon />
            <span>히스토리</span>
          </button>
        </nav>
        <div className="profile-dot" aria-label="사용자 프로필">
          윤
        </div>
      </aside>
      <main className="workspace">{children}</main>
    </div>
  );
}
