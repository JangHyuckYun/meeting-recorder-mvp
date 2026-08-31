//! Transcript export rendering: one fixture (2 speakers, 3 segments) per format.

use desktop_lib::export::{render, safe_filename, ExportFormat};
use desktop_lib::models::TranscriptSegment;
use std::collections::BTreeMap;
use uuid::Uuid;

fn fixture() -> Vec<TranscriptSegment> {
    let recording_id = Uuid::new_v4();
    [
        (0, 1_500, "화자 1", "안녕하세요"),
        (1_500, 3_250, "화자 2", "반갑습니다"),
        (3_600_000, 3_601_000, "화자 1", "그럼 시작하죠"),
    ]
    .into_iter()
    .map(
        |(start_ms, end_ms, speaker_label, text)| TranscriptSegment {
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
fn renders_srt_with_indices_and_comma_millis() {
    let out = render("주간 회의", &fixture(), &BTreeMap::new(), ExportFormat::Srt);
    assert!(out.starts_with("1\n00:00:00,000 --> 00:00:01,500\n화자 1: 안녕하세요\n"));
    assert!(out.contains("2\n00:00:01,500 --> 00:00:03,250\n화자 2: 반갑습니다"));
    // Hours must roll over rather than overflowing the minutes field.
    assert!(out.contains("3\n01:00:00,000 --> 01:00:01,000\n화자 1: 그럼 시작하죠"));
}

#[test]
fn renders_vtt_with_header_and_dot_millis() {
    let out = render("주간 회의", &fixture(), &BTreeMap::new(), ExportFormat::Vtt);
    assert!(out.starts_with("WEBVTT\n\n"));
    assert!(out.contains("00:00:00.000 --> 00:00:01.500\n화자 1: 안녕하세요"));
    assert!(!out.contains(','), "VTT must not use SRT comma millis");
}

#[test]
fn renders_md_with_title_heading() {
    let out = render("주간 회의", &fixture(), &BTreeMap::new(), ExportFormat::Md);
    assert!(out.starts_with("# 주간 회의\n\n"));
    assert!(out.contains("**화자 2** `00:00:01`\n\n반갑습니다"));
}

#[test]
fn renders_txt_as_one_line_per_segment() {
    let out = render("주간 회의", &fixture(), &BTreeMap::new(), ExportFormat::Txt);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "[00:00:00] 화자 1: 안녕하세요");
    assert_eq!(lines[2], "[01:00:00] 화자 1: 그럼 시작하죠");
}

#[test]
fn export_applies_speaker_name_overrides() {
    let names = BTreeMap::from([("화자 1".to_string(), "김민지".to_string())]);
    let out = render("주간 회의", &fixture(), &names, ExportFormat::Txt);
    assert!(out.contains("김민지: 안녕하세요"));
    // Speakers without an override keep the diarization label.
    assert!(out.contains("화자 2: 반갑습니다"));
}

#[test]
fn unknown_format_is_a_typed_error_and_filenames_are_sanitized() {
    assert!(ExportFormat::from_str("docx").is_err());
    assert_eq!(ExportFormat::from_str("SRT").unwrap(), ExportFormat::Srt);

    assert_eq!(
        safe_filename("a/b:c", "abc12345", ExportFormat::Vtt),
        "a_b_c-abc12345.vtt"
    );
    assert_eq!(
        safe_filename("   ", "abc12345", ExportFormat::Md),
        "transcript-abc12345.md"
    );
}
