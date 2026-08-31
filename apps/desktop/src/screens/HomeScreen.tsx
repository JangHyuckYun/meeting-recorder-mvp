/** S1 — Home (notes list). Wraps HistoryScreen's list for now; owns nothing else. */
import { HistoryScreen } from "./HistoryScreen";

interface HomeScreenProps {
  /** Called with a recording's id when its row is opened (navigates to S4). */
  onOpenNote: (recordingId: string) => void;
}

export function HomeScreen({ onOpenNote }: HomeScreenProps) {
  return <HistoryScreen onOpenRecording={onOpenNote} />;
}
