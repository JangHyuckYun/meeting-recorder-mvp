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
import { HistoryScreen } from "./screens/HistoryScreen";
import { ImportScreen } from "./screens/ImportScreen";
import { LiveRecordingScreen } from "./screens/LiveRecordingScreen";

type AppView = "live" | "history" | "import";

function App() {
  const [activeView, setActiveView] = useState<AppView>("live");

  return (
    <AppShell activeView={activeView} onNavigate={setActiveView}>
      {activeView === "live" ? (
        <LiveRecordingScreen />
      ) : activeView === "history" ? (
        <HistoryScreen />
      ) : (
        <ImportScreen />
      )}
    </AppShell>
  );
}

export default App;
