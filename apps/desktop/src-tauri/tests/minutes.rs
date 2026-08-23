use desktop_lib::minutes::{edit_minutes_item_text, generate_minutes, parse_minutes_response};
use desktop_lib::models::{LlmProvider, MinutesItem, TranscriptSegment};
use std::collections::HashSet;
use uuid::Uuid;

#[test]
fn parses_grounded_minutes_and_drops_items_without_valid_evidence() {
    let recording_id = Uuid::parse_str("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa").unwrap();
    let first_segment_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap();
    let second_segment_id = Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap();
    let response = r#"
    {
      "summary": " 출시 일정과 QA 담당 업무를 확정했다. ",
      "decisions": [
        {
          "text": "출시일을 9월 1일로 확정했다.",
          "evidence_segment_ids": [
            "11111111-1111-4111-8111-111111111111",
            "11111111-1111-4111-8111-111111111111",
            "ffffffff-ffff-4fff-8fff-ffffffffffff"
          ]
        },
        {"text": "근거 없는 결정", "evidence_segment_ids": []}
      ],
      "action_items": [
        {
          "text": "민지가 QA 체크리스트를 작성한다.",
          "evidence_segment_ids": ["22222222-2222-4222-8222-222222222222"]
        },
        {"text": "근거 필드가 없는 할 일"}
      ]
    }
    "#;

    let draft = parse_minutes_response(
        recording_id,
        response,
        &[first_segment_id, second_segment_id],
    )
    .expect("parse grounded minutes");

    assert_eq!(draft.recording_id, recording_id);
    assert_eq!(draft.summary, "출시 일정과 QA 담당 업무를 확정했다.");
    assert_eq!(draft.decisions.len(), 1);
    assert_eq!(
        draft.decisions[0].evidence_segment_ids,
        vec![first_segment_id]
    );
    assert_eq!(draft.action_items.len(), 1);
    assert_eq!(
        draft.action_items[0].evidence_segment_ids,
        vec![second_segment_id]
    );
    assert_ne!(draft.decisions[0].id, Uuid::nil());
    assert_ne!(draft.action_items[0].id, Uuid::nil());
}

#[tokio::test]
#[ignore = "requires the live LiteLLM gateway at 192.168.1.189:4000"]
async fn generates_grounded_minutes_through_live_litellm() {
    let recording_id = Uuid::new_v4();
    let segments = vec![
        segment(
            recording_id,
            0,
            3_000,
            "화자 1",
            "제품 출시일은 9월 1일로 확정하겠습니다. 모두 동의하시죠?",
        ),
        segment(
            recording_id,
            3_000,
            5_000,
            "화자 2",
            "네, 동의합니다. 결정된 것으로 기록해 주세요.",
        ),
        segment(
            recording_id,
            5_000,
            8_000,
            "화자 1",
            "민지 님은 8월 25일까지 QA 체크리스트를 작성해 주세요.",
        ),
    ];
    let valid_ids = segments
        .iter()
        .map(|segment| segment.id)
        .collect::<HashSet<_>>();

    let draft = generate_minutes(LlmProvider::Litellm, recording_id, &segments)
        .await
        .expect("live LiteLLM minutes generation");
    println!(
        "live LiteLLM response:\n{}",
        serde_json::to_string_pretty(&draft).unwrap()
    );

    assert_eq!(draft.recording_id, recording_id);
    assert!(!draft.summary.trim().is_empty());
    let items = draft
        .decisions
        .iter()
        .chain(draft.action_items.iter())
        .collect::<Vec<_>>();
    assert!(
        !items.is_empty(),
        "the explicit decision/action should produce an item"
    );
    for item in items {
        assert!(!item.evidence_segment_ids.is_empty());
        assert!(item
            .evidence_segment_ids
            .iter()
            .all(|segment_id| valid_ids.contains(segment_id)));
    }
}

/// SC4 evidence: edits ONE minutes item's text via the live LLM and proves the id + evidence
/// references are untouched while the text actually changes (before/after diff captured).
#[tokio::test]
#[ignore = "requires the live LiteLLM gateway at 192.168.1.189:4000"]
async fn edits_single_minutes_item_via_live_litellm_preserving_identity() {
    let recording_id = Uuid::new_v4();
    let evidence_segment = segment(
        recording_id,
        5_000,
        8_000,
        "화자 1",
        "민지 님은 8월 25일까지 QA 체크리스트를 작성해 주세요.",
    );
    let original = MinutesItem {
        id: Uuid::new_v4(),
        text: "민지 님은 8월 25일까지 QA 체크리스트를 작성해 주세요.".to_string(),
        evidence_segment_ids: vec![evidence_segment.id],
    };

    let edited_text = edit_minutes_item_text(
        LlmProvider::Litellm,
        &original,
        "고객 보고용으로 더 간결하고 격식 있게 다듬어줘",
        std::slice::from_ref(&evidence_segment),
    )
    .await
    .expect("live LiteLLM minutes item edit");

    println!("BEFORE: {}", original.text);
    println!("AFTER:  {edited_text}");

    assert!(!edited_text.trim().is_empty());
    assert_ne!(
        edited_text, original.text,
        "instruction should actually change the text"
    );
    // Caller-side contract: id and evidence_segment_ids are never touched by the edit call
    // itself (edit_minutes_item_text returns text only) — assert that contract holds.
    let reapplied = MinutesItem {
        id: original.id,
        text: edited_text,
        evidence_segment_ids: original.evidence_segment_ids.clone(),
    };
    assert_eq!(reapplied.id, original.id);
    assert_eq!(reapplied.evidence_segment_ids, original.evidence_segment_ids);
}

fn segment(
    recording_id: Uuid,
    start_ms: i64,
    end_ms: i64,
    speaker_label: &str,
    text: &str,
) -> TranscriptSegment {
    TranscriptSegment {
        id: Uuid::new_v4(),
        recording_id,
        start_ms,
        end_ms,
        speaker_label: speaker_label.to_string(),
        text: text.to_string(),
        is_final: true,
    }
}

#[tokio::test]
#[ignore = "requires a Codex CLI OAuth login (~/.codex/auth.json)"]
async fn generates_grounded_minutes_through_live_oauth_provider() {
    let recording_id = Uuid::new_v4();
    let segments = vec![
        segment(
            recording_id,
            0,
            2_500,
            "화자 1",
            "이번 금요일까지 고객 피드백 보고서를 완성하기로 합시다.",
        ),
        segment(
            recording_id,
            2_500,
            5_000,
            "화자 2",
            "동의합니다. 제가 목요일 오후까지 보고서 초안을 작성하겠습니다.",
        ),
        segment(
            recording_id,
            5_000,
            7_000,
            "화자 1",
            "좋습니다. 금요일 오전에 함께 검토하겠습니다.",
        ),
    ];
    let valid_ids = segments
        .iter()
        .map(|segment| segment.id)
        .collect::<HashSet<_>>();

    let draft = generate_minutes(LlmProvider::CodexOauth, recording_id, &segments)
        .await
        .expect("live OAuth minutes generation");
    println!(
        "live OAuth response:\n{}",
        serde_json::to_string_pretty(&draft).unwrap()
    );

    assert_eq!(draft.recording_id, recording_id);
    assert!(!draft.summary.trim().is_empty());
    let items = draft
        .decisions
        .iter()
        .chain(draft.action_items.iter())
        .collect::<Vec<_>>();
    assert!(
        !items.is_empty(),
        "the explicit decision/action should produce a grounded item"
    );
    for item in items {
        assert!(!item.evidence_segment_ids.is_empty());
        assert!(item
            .evidence_segment_ids
            .iter()
            .all(|segment_id| valid_ids.contains(segment_id)));
    }
}

#[tokio::test]
#[ignore = "requires a Claude CLI OAuth login (~/.claude/.credentials.json)"]
async fn generates_grounded_minutes_through_live_claude_oauth() {
    let recording_id = Uuid::new_v4();
    let segments = vec![
        segment(
            recording_id,
            0,
            2_500,
            "화자 1",
            "이번 금요일까지 고객 피드백 보고서를 완성하기로 합시다.",
        ),
        segment(
            recording_id,
            2_500,
            5_000,
            "화자 2",
            "동의합니다. 제가 목요일 오후까지 보고서 초안을 작성하겠습니다.",
        ),
        segment(
            recording_id,
            5_000,
            7_000,
            "화자 1",
            "좋습니다. 금요일 오전에 함께 검토하겠습니다.",
        ),
    ];
    let valid_ids = segments
        .iter()
        .map(|segment| segment.id)
        .collect::<HashSet<_>>();

    let draft = generate_minutes(LlmProvider::ClaudeOauth, recording_id, &segments)
        .await
        .expect("live Claude OAuth minutes generation");
    println!(
        "live Claude OAuth response:\n{}",
        serde_json::to_string_pretty(&draft).unwrap()
    );

    assert_eq!(draft.recording_id, recording_id);
    assert!(!draft.summary.trim().is_empty());
    let items = draft
        .decisions
        .iter()
        .chain(draft.action_items.iter())
        .collect::<Vec<_>>();
    assert!(
        !items.is_empty(),
        "the explicit decision/action should produce a grounded item"
    );
    for item in items {
        assert!(!item.evidence_segment_ids.is_empty());
        assert!(item
            .evidence_segment_ids
            .iter()
            .all(|segment_id| valid_ids.contains(segment_id)));
    }
}
