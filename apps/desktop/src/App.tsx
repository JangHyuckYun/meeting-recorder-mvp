import { useState } from "react";
import "./App.css";
import { AppShell } from "./components/AppShell";
import { HistoryScreen } from "./screens/HistoryScreen";
import { LiveRecordingScreen } from "./screens/LiveRecordingScreen";

type AppView = "live" | "history";

function App() {
  const [activeView, setActiveView] = useState<AppView>("live");

  return (
    <AppShell activeView={activeView} onNavigate={setActiveView}>
      {activeView === "live" ? <LiveRecordingScreen /> : <HistoryScreen />}
    </AppShell>
  );
}

export default App;
