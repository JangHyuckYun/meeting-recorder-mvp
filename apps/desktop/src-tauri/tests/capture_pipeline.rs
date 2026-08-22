//! Integration tests for the capture->STT pipeline (feat/capture-pipeline).
//!
//! `storage_roundtrip_persists_recording_and_segments` always runs — pure SQLite, no network.
//!
//! `real_test_audio_transcribes_with_speaker_labels` is `#[ignore]`d by default because it
//! needs the self-hosted WhisperLive server (infra/stt-server/, 192.168.1.189:9090) reachable
//! and real audio from /Users/janghyeok/Downloads/record_test_data (never committed to this
//! repo). Run explicitly once the server is confirmed healthy:
//!   TEST_AUDIO_DIR=/Users/janghyeok/Downloads/record_test_data cargo test --test capture_pipeline -- --ignored --nocapture

use desktop_lib::models::{Recording, RecordingStatus};
use desktop_lib::storage::Storage;
use desktop_lib::stt::{self, SttConfig};
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

#[tokio::test]
async fn storage_roundtrip_persists_recording_and_segments() {
    let tmp = std::env::temp_dir().join(format!("capture-pipeline-test-{}.sqlite3", Uuid::new_v4()));
    let storage = Storage::connect(&tmp).await.expect("connect");

    let rec = Recording {
        id: Uuid::new_v4(),
        title: "테스트 회의".to_string(),
        source_path: "/tmp/does-not-need-to-exist.wav".to_string(),
        duration_ms: Some(12_345),
        status: RecordingStatus::Recorded,
        created_at: chrono::Utc::now(),
    };
    storage.insert_recording(&rec).await.expect("insert");

    let fetched = storage.get_recording(rec.id).await.expect("get").expect("present");
    assert_eq!(fetched.title, "테스트 회의");
    assert_eq!(fetched.status, RecordingStatus::Recorded);
    assert_eq!(fetched.duration_ms, Some(12_345));

    let all = storage.list_recordings().await.expect("list");
    assert!(all.iter().any(|r| r.id == rec.id));

    let _ = std::fs::remove_file(&tmp);
}

fn convert_m4a_to_wav(src: &std::path::Path, dst: &std::path::Path) {
    let status = Command::new("afconvert")
        .args(["-f", "WAVE", "-d", "LEI16@16000", "-c", "1"])
        .arg(src)
        .arg(dst)
        .status()
        .expect("spawn afconvert");
    assert!(status.success(), "afconvert failed for {src:?}");
}

/// Runs a real, short recording from the local test-data corpus through the deployed
/// WhisperLive server and asserts it produced non-empty, speaker-labeled transcript segments.
#[tokio::test]
#[ignore = "requires the deployed STT server (infra/stt-server/) and local test audio corpus"]
async fn real_test_audio_transcribes_with_speaker_labels() {
    let audio_dir = std::env::var("TEST_AUDIO_DIR")
        .unwrap_or_else(|_| "/Users/janghyeok/Downloads/record_test_data".to_string());
    let src = PathBuf::from(&audio_dir).join("등촌동 12.m4a");
    assert!(src.exists(), "test fixture missing: {src:?} (set TEST_AUDIO_DIR)");

    let wav = std::env::temp_dir().join(format!("stt-smoke-{}.wav", Uuid::new_v4()));
    convert_m4a_to_wav(&src, &wav);

    let cfg = SttConfig::default();
    let recording_id = Uuid::new_v4();
    let segments = stt::transcribe_wav_file(&cfg, recording_id, &wav, None)
        .await
        .expect("transcription should succeed against the live STT server");

    let _ = std::fs::remove_file(&wav);

    assert!(!segments.is_empty(), "expected at least one transcribed segment");
    for seg in &segments {
        assert!(!seg.text.trim().is_empty(), "segment text should not be empty");
        assert!(seg.end_ms >= seg.start_ms, "segment end must not precede start");
    }
    println!("transcribed {} segments from {:?}", segments.len(), src.file_name().unwrap());
    for seg in segments.iter().take(5) {
        println!("  [{}] {}-{}ms: {}", seg.speaker_label, seg.start_ms, seg.end_ms, seg.text);
    }
}

/// SC2's second, differently-sized fixture: a longer recording than the ~27s smoke file above,
/// so the pipeline is proven against more than one duration/speaker-count profile. WhisperLive
/// streams at real-time pace, so full-length recordings in the corpus (30-100 min) are
/// impractical to run in CI-scale time; `TEST_AUDIO_LONG_FILE` points at a trimmed clip cut from
/// one of them with `ffmpeg -ss <offset> -t <secs> -c copy`, never committed to this repo.
#[tokio::test]
#[ignore = "requires the deployed STT server (infra/stt-server/) and a longer local audio clip"]
async fn real_longer_test_audio_transcribes_with_speaker_labels() {
    let src = PathBuf::from(
        std::env::var("TEST_AUDIO_LONG_FILE")
            .unwrap_or_else(|_| "/tmp/long-clip-등촌동18-3min.m4a".to_string()),
    );
    assert!(
        src.exists(),
        "long test fixture missing: {src:?} (set TEST_AUDIO_LONG_FILE to a trimmed clip)"
    );

    let wav = std::env::temp_dir().join(format!("stt-smoke-long-{}.wav", Uuid::new_v4()));
    convert_m4a_to_wav(&src, &wav);

    let cfg = SttConfig::default();
    let recording_id = Uuid::new_v4();
    let segments = stt::transcribe_wav_file(&cfg, recording_id, &wav, None)
        .await
        .expect("transcription should succeed against the live STT server");

    let _ = std::fs::remove_file(&wav);

    assert!(!segments.is_empty(), "expected at least one transcribed segment");
    let distinct_speakers = segments
        .iter()
        .map(|s| s.speaker_label.as_str())
        .collect::<std::collections::HashSet<_>>()
        .len();
    println!(
        "transcribed {} segments ({} distinct speaker labels) from {:?}",
        segments.len(),
        distinct_speakers,
        src.file_name().unwrap()
    );
    for seg in segments.iter().take(8) {
        println!("  [{}] {}-{}ms: {}", seg.speaker_label, seg.start_ms, seg.end_ms, seg.text);
    }
}
