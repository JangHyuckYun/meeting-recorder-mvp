//! Grounded Q&A prompt/response contract. The network call is the only untested part;
//! the prompt built for the provider and the best-effort citation parse are both pure.

use desktop_lib::minutes::{ask_user_prompt, parse_ask_response};
use desktop_lib::models::{MinutesDraft, MinutesItem, TranscriptSegment};
use uuid::Uuid;

fn fixture() -> Vec<TranscriptSegment> {
    let recording_id = Uuid::new_v4();
    [
        ("화자 1", "출시일은 9월 1일로 하죠", 0, 2_000),
        ("화자 2", "QA는 제가 맡겠습니다", 2_000, 4_000),
    ]
    .into_iter()
    .map(
        |(speaker_label, text, start_ms, end_ms)| TranscriptSegment {
            id: Uuid::new_v4(),
            recording_id,
            start_ms,
            end_ms,
            speaker_label: speaker_label.to_string(),
            text: text.to_string(),
            is_final: true,
        },
    )
    .collect()
}

#[test]
fn ask_prompt_carries_the_question_transcript_and_minutes() {
    let segments = fixture();
    let draft = MinutesDraft {
        recording_id: segments[0].recording_id,
        summary: "출시 일정을 확정했다.".to_string(),
        decisions: vec![MinutesItem {
            id: Uuid::new_v4(),
            text: "출시일 9월 1일".to_string(),
            evidence_segment_ids: vec![segments[0].id],
        }],
        action_items: vec![],
        updated_at: chrono::Utc::now(),
    };

    let prompt = ask_user_prompt("출시일이 언제인가요?", &segments, Some(&draft)).unwrap();
    let value: serde_json::Value = serde_json::from_str(&prompt).unwrap();
    assert_eq!(value["question"], "출시일이 언제인가요?");
    assert_eq!(value["transcript"].as_array().unwrap().len(), 2);
    // Segment ids must be quotable by the model, so they have to reach the prompt.
    assert_eq!(value["transcript"][0]["id"], segments[0].id.to_string());
    assert_eq!(value["minutes"]["summary"], "출시 일정을 확정했다.");

    // Minutes are optional — a recording with no draft yet is still answerable.
    let without = ask_user_prompt("질문", &segments, None).unwrap();
    let value: serde_json::Value = serde_json::from_str(&without).unwrap();
    assert!(value["minutes"].is_null());
}

#[test]
fn ask_response_resolves_cited_segments_to_timestamped_sources() {
    let segments = fixture();
    let response = format!(
        r#"{{"answer": "9월 1일입니다.", "source_segment_ids": ["{}"]}}"#,
        segments[0].id
    );

    let parsed = parse_ask_response(&response, &segments);
    assert_eq!(parsed.answer, "9월 1일입니다.");
    assert_eq!(parsed.sources.len(), 1);
    assert_eq!(parsed.sources[0].segment_id, segments[0].id);
    assert_eq!(parsed.sources[0].start_ms, 0);
    assert_eq!(parsed.sources[0].end_ms, 2_000);
}

#[test]
fn ask_response_drops_invented_citations_without_failing() {
    let segments = fixture();
    let response = format!(
        r#"{{"answer": "답변", "source_segment_ids": ["{}", "not-a-uuid", "{}"]}}"#,
        Uuid::new_v4(),
        segments[1].id
    );

    let parsed = parse_ask_response(&response, &segments);
    assert_eq!(parsed.answer, "답변");
    assert_eq!(parsed.sources.len(), 1, "only the real segment survives");
    assert_eq!(parsed.sources[0].segment_id, segments[1].id);
}

#[test]
fn ask_response_never_errors_on_unparseable_output() {
    let segments = fixture();

    // Missing citations: answer stands, sources empty.
    let parsed = parse_ask_response(r#"{"answer": "  답변만  "}"#, &segments);
    assert_eq!(parsed.answer, "답변만");
    assert!(parsed.sources.is_empty());

    // Not JSON at all: the prose itself is the answer.
    let parsed = parse_ask_response("그냥 평문 답변", &segments);
    assert_eq!(parsed.answer, "그냥 평문 답변");
    assert!(parsed.sources.is_empty());
}
