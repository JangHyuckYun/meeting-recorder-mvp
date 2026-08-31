import { useState } from "react";
import "./styles/global.css";
import "./styles/base.css";
import "./styles/shell.css";
import "./styles/live.css";
import "./styles/history.css";
import "./styles/import.css";
import "./styles/minutes.css";
import "./styles/settings.css";
import { AppShell } from "./components/AppShell";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { SettingsModal } from "./components/SettingsModal";
import { AskScreen } from "./screens/AskScreen";
import { ExportScreen } from "./screens/ExportScreen";
import { HistoryScreen } from "./screens/HistoryScreen";
import { ImportScreen } from "./screens/ImportScreen";
import { LiveRecordingScreen } from "./screens/LiveRecordingScreen";
import { ShareScreen } from "./screens/ShareScreen";
import { TemplatesScreen } from "./screens/TemplatesScreen";
import type { Route } from "./routes";

interface AppProps {
  /** Testability hook: renders the app already on a given route. */
  initialRoute?: Route;
}

function renderScreen(route: Route, navigate: (route: Route) => void) {
  switch (route) {
    case "s1": // Home — notes list
      return <HistoryScreen />;
    case "s2": // Prepare recording
      return <LiveRecordingScreen />;
    case "s3": // Live recording
      return <LiveRecordingScreen />;
    case "s4": // Note detail (reuses HistoryScreen's own list/detail toggle)
      return <HistoryScreen />;
    case "s5": // Templates
      return <TemplatesScreen />;
    case "s6": // Import
      return <ImportScreen />;
    case "s7": // Export
      return <ExportScreen />;
    case "s8": // Settings
      return <SettingsModal open onClose={() => navigate("s1")} />;
    case "s9": // Ask
      return <AskScreen />;
    case "s10": // Share
      return <ShareScreen />;
  }
}

function App({ initialRoute = "s1" }: AppProps) {
  const [route, setRoute] = useState<Route>(initialRoute);

  return (
    <AppShell activeRoute={route} onNavigate={setRoute}>
      <div className="screen-container" data-testid={`screen-${route}`}>
        <ErrorBoundary>{renderScreen(route, setRoute)}</ErrorBoundary>
      </div>
    </AppShell>
  );
}

export default App;
