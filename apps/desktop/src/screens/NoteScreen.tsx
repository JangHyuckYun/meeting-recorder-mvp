/** S4 — Note detail. Wraps HistoryScreen's detail/MinutesView path for now. */
import { Button } from "@/components/ui";
import { HistoryScreen } from "./HistoryScreen";

interface NoteScreenProps {
  recordingId: string;
  /** Navigates to S7/S9/S10 with this note's recordingId. */
  onExport: (recordingId: string) => void;
  onAsk: (recordingId: string) => void;
  onShare: (recordingId: string) => void;
}

export function NoteScreen({ recordingId, onExport, onAsk, onShare }: NoteScreenProps) {
  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%" }}>
      <div
        style={{ display: "flex", flex: "none", gap: 8, justifyContent: "flex-end", padding: "8px 12px" }}
      >
        <Button variant="outline" size="sm" onClick={() => onAsk(recordingId)}>
          질문하기
        </Button>
        <Button variant="outline" size="sm" onClick={() => onExport(recordingId)}>
          내보내기
        </Button>
        <Button size="sm" onClick={() => onShare(recordingId)}>
          공유
        </Button>
      </div>
      <div style={{ flex: 1, minHeight: 0, overflow: "hidden" }}>
        <HistoryScreen initialRecordingId={recordingId} />
      </div>
    </div>
  );
}
