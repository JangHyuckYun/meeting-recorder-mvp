/** S7 — Export. Placeholder shell owned by whichever agent builds this screen next. */
interface ExportScreenProps {
  recordingId: string;
}

export function ExportScreen({ recordingId }: ExportScreenProps) {
  return (
    <section className="placeholder-screen">
      <p className="import-eyebrow">EXPORT</p>
      <h1>내보내기</h1>
      <p>내보내기 화면은 준비 중입니다.</p>
      <p data-numeric>recording: {recordingId}</p>
    </section>
  );
}
