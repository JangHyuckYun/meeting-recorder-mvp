/** S2 — Prepare recording. Placeholder shell owned by whichever agent builds this screen next. */
import { Button } from "@/components/ui";

interface PrepareScreenProps {
  /** Called when the user is ready to start recording (navigates to S3). */
  onStart: () => void;
}

export function PrepareScreen({ onStart }: PrepareScreenProps) {
  return (
    <section className="placeholder-screen">
      <p className="import-eyebrow">PREPARE</p>
      <h1>녹음 준비</h1>
      <p>회의 준비 화면은 준비 중입니다.</p>
      <Button onClick={onStart}>바로 녹음 시작</Button>
    </section>
  );
}
