/** S9 — Ask (note Q&A). Placeholder shell owned by whichever agent builds this screen next. */
interface AskScreenProps {
  recordingId: string;
}

export function AskScreen({ recordingId }: AskScreenProps) {
  return (
    <section className="placeholder-screen">
      <p className="import-eyebrow">ASK</p>
      <h1>질문하기</h1>
      <p>노트에게 질문하는 화면은 준비 중입니다.</p>
      <p data-numeric>recording: {recordingId}</p>
    </section>
  );
}
