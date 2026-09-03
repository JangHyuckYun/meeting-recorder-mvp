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
import { HomeScreen } from "./screens/HomeScreen";
import { ImportScreen } from "./screens/ImportScreen";
import { LiveRecordingScreen } from "./screens/LiveRecordingScreen";
import { NoteScreen } from "./screens/NoteScreen";
import { PrepareScreen } from "./screens/PrepareScreen";
import { ShareScreen } from "./screens/ShareScreen";
import { TemplatesScreen } from "./screens/TemplatesScreen";
import type { Route } from "./routes";

interface AppProps {
  /** Testability hook: renders the app already on a given route. */
  initialRoute?: Route;
}

function renderScreen(route: Route, navigate: (route: Route) => void) {
  switch (route.id) {
    case "s1": // Home — notes list
      return <HomeScreen onOpenNote={(recordingId) => navigate({ id: "s4", recordingId })} onOpenImport={() => navigate({ id: "s6" })} />;
    case "s2": // Prepare recording
      return <PrepareScreen onStart={() => navigate({ id: "s3" })} />;
    case "s3": // Live recording
      return <LiveRecordingScreen />;
    case "s4": // Note detail
      return (
        <NoteScreen
          recordingId={route.recordingId}
          onExport={(recordingId) => navigate({ id: "s7", recordingId })}
          onAsk={(recordingId) => navigate({ id: "s9", recordingId })}
          onShare={(recordingId) => navigate({ id: "s10", recordingId })}
        />
      );
    case "s5": // Templates
      return <TemplatesScreen />;
    case "s6": // Import
      return <ImportScreen />;
    case "s7": // Export
      return <ExportScreen recordingId={route.recordingId} />;
    case "s8": // Settings
      return <SettingsModal open onClose={() => navigate({ id: "s1" })} />;
    case "s9": // Ask
      return <AskScreen recordingId={route.recordingId} />;
    case "s10": // Share
      return <ShareScreen recordingId={route.recordingId} />;
  }
}

function App({ initialRoute = { id: "s1" } }: AppProps) {
  const [route, setRoute] = useState<Route>(initialRoute);

  return (
    <AppShell activeRouteId={route.id} onNavigate={(id) => setRoute({ id })}>
      <div className="screen-container" data-testid={`screen-${route.id}`}>
        <ErrorBoundary>{renderScreen(route, setRoute)}</ErrorBoundary>
      </div>
    </AppShell>
  );
}

export default App;
