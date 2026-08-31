//! Recording lifecycle storage contract: deletion, folders, and speaker name overrides.

use desktop_lib::error::AppError;
use desktop_lib::models::{Recording, RecordingStatus, TranscriptSegment};
use desktop_lib::storage::Storage;
use uuid::Uuid;

async fn storage(name: &str) -> (tempfile::TempDir, Storage) {
    let dir = tempfile::tempdir().expect("tempdir should create");
    let storage = Storage::connect(&dir.path().join(name))
        .await
        .expect("storage should connect and migrate");
    (dir, storage)
}

fn recording(title: &str, source_path: &str) -> Recording {
    Recording {
        id: Uuid::new_v4(),
        title: title.to_string(),
        source_path: source_path.to_string(),
        duration_ms: Some(1_000),
        status: RecordingStatus::Recorded,
        created_at: chrono::Utc::now(),
    }
}

fn segment(recording_id: Uuid) -> TranscriptSegment {
    TranscriptSegment {
        id: Uuid::new_v4(),
        recording_id,
        start_ms: 0,
        end_ms: 500,
        speaker_label: "화자 1".to_string(),
        text: "안녕하세요".to_string(),
        is_final: true,
    }
}

#[tokio::test]
async fn delete_recording_removes_row_and_child_segments() {
    let (_dir, storage) = storage("delete-test.sqlite3").await;
    let rec = recording("삭제 대상", "/tmp/does-not-exist.wav");
    storage.insert_recording(&rec).await.expect("insert");
    storage
        .insert_segments(&[segment(rec.id)])
        .await
        .expect("segments insert");

    storage.delete_recording(rec.id).await.expect("delete");

    assert!(storage.get_recording(rec.id).await.expect("get").is_none());
    assert!(storage
        .list_segments(rec.id)
        .await
        .expect("segments should still query")
        .is_empty());
}

#[tokio::test]
async fn delete_unknown_recording_returns_not_found_instead_of_panicking() {
    let (_dir, storage) = storage("delete-unknown-test.sqlite3").await;

    let error = storage
        .delete_recording(Uuid::new_v4())
        .await
        .expect_err("deleting an unknown id must be a typed error");
    assert!(matches!(error, AppError::NotFound(_)), "got {error:?}");
}
