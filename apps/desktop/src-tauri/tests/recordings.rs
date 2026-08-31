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
        folder_id: None,
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
async fn folder_assignment_roundtrips_and_survives_folder_deletion() {
    let (_dir, storage) = storage("folders-test.sqlite3").await;
    let rec = recording("주간 회의", "/tmp/a.wav");
    storage.insert_recording(&rec).await.expect("insert");

    assert!(storage.list_folders().await.expect("list").is_empty());
    let folder = storage
        .create_folder("  프로젝트 A  ")
        .await
        .expect("create");
    assert_eq!(folder.name, "프로젝트 A");
    assert_eq!(storage.list_folders().await.expect("list").len(), 1);

    storage
        .assign_recording_folder(rec.id, Some(folder.id))
        .await
        .expect("assign");
    let listed = storage.list_recordings().await.expect("list recordings");
    assert_eq!(listed[0].folder_id, Some(folder.id));

    // Unfiling is an explicit None, not a separate command.
    storage
        .assign_recording_folder(rec.id, None)
        .await
        .expect("unassign");
    assert_eq!(
        storage
            .get_recording(rec.id)
            .await
            .expect("get")
            .unwrap()
            .folder_id,
        None
    );

    // Deleting a folder unfiles its recordings instead of deleting them.
    storage
        .assign_recording_folder(rec.id, Some(folder.id))
        .await
        .expect("reassign");
    storage
        .delete_folder(folder.id)
        .await
        .expect("delete folder");
    let survivor = storage
        .get_recording(rec.id)
        .await
        .expect("get")
        .expect("recording must survive folder deletion");
    assert_eq!(survivor.folder_id, None);
}

#[tokio::test]
async fn folder_and_recording_assignment_errors_are_typed() {
    let (_dir, storage) = storage("folders-error-test.sqlite3").await;

    assert!(matches!(
        storage.create_folder("   ").await.expect_err("blank name"),
        AppError::InvalidState(_)
    ));
    assert!(matches!(
        storage
            .delete_folder(Uuid::new_v4())
            .await
            .expect_err("unknown folder"),
        AppError::NotFound(_)
    ));
    assert!(matches!(
        storage
            .assign_recording_folder(Uuid::new_v4(), None)
            .await
            .expect_err("unknown recording"),
        AppError::NotFound(_)
    ));
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
