import { type ReactNode } from "react";
import { cn } from "@/lib/utils";
import type { Route } from "@/routes";

interface AppShellProps {
  activeRoute: Route;
  children: ReactNode;
  onNavigate: (route: Route) => void;
}

function HomeIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M4 11.5 12 4l8 7.5M6 10v9h12v-9" />
    </svg>
  );
}

function MicIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <rect x="8" y="3" width="8" height="13" rx="4" />
      <path d="M5 11a7 7 0 0 0 14 0M12 18v3M9 21h6" />
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

function TemplateIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <rect x="4" y="4" width="16" height="16" rx="2" />
      <path d="M4 10h16M10 10v10" />
    </svg>
  );
}

function SettingsIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1 1.55V21a2 2 0 0 1-4 0v-.09a1.7 1.7 0 0 0-1-1.55 1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.7 1.7 0 0 0 .34-1.87 1.7 1.7 0 0 0-1.55-1H3a2 2 0 0 1 0-4h.09a1.7 1.7 0 0 0 1.55-1 1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.7 1.7 0 0 0 1.87.34H9a1.7 1.7 0 0 0 1-1.55V3a2 2 0 0 1 4 0v.09a1.7 1.7 0 0 0 1 1.55 1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.7 1.7 0 0 0-.34 1.87V9a1.7 1.7 0 0 0 1.55 1H21a2 2 0 0 1 0 4h-.09a1.7 1.7 0 0 0-1.55 1z" />
    </svg>
  );
}

const NAV_ITEMS: { route: Route; label: string; title: string; Icon: () => ReactNode }[] = [
  { route: "s1", label: "홈", title: "홈", Icon: HomeIcon },
  { route: "s3", label: "실시간", title: "실시간 기록", Icon: MicIcon },
  { route: "s6", label: "가져오기", title: "가져오기", Icon: ImportIcon },
  { route: "s5", label: "템플릿", title: "템플릿", Icon: TemplateIcon },
  { route: "s8", label: "설정", title: "설정", Icon: SettingsIcon },
];

export function AppShell({ activeRoute, children, onNavigate }: AppShellProps) {
  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="주요 메뉴">
        <div className="brand" aria-label="Minute">
          M
        </div>
        <nav className="sidebar-nav">
          {NAV_ITEMS.map(({ route, label, title, Icon }) => (
            <button
              key={route}
              className={cn("nav-button", activeRoute === route && "active")}
              type="button"
              title={title}
              aria-label={title}
              aria-current={activeRoute === route ? "page" : undefined}
              onClick={() => onNavigate(route)}
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
            onClick={() => onNavigate("s8")}
          >
            윤
          </button>
        </div>
      </aside>
      <main className="workspace">{children}</main>
    </div>
  );
}
