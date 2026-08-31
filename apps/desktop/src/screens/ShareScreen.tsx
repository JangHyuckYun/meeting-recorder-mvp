/** S10 — Share. Placeholder shell owned by whichever agent builds this screen next. */
interface ShareScreenProps {
  recordingId: string;
}

export function ShareScreen({ recordingId }: ShareScreenProps) {
  return (
    <section className="placeholder-screen">
      <p className="import-eyebrow">SHARE</p>
      <h1>공유</h1>
      <p>공유 화면은 준비 중입니다.</p>
      <p data-numeric>recording: {recordingId}</p>
    </section>
  );
}
