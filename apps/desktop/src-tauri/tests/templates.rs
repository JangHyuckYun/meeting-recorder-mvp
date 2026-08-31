//! Minutes template storage contract and the prompt-customization point it feeds.

use desktop_lib::error::AppError;
use desktop_lib::minutes::minutes_system_prompt;
use desktop_lib::storage::Storage;
use uuid::Uuid;

async fn storage(name: &str) -> (tempfile::TempDir, Storage) {
    let dir = tempfile::tempdir().expect("tempdir should create");
    let storage = Storage::connect(&dir.path().join(name))
        .await
        .expect("storage should connect and migrate");
    (dir, storage)
}

#[tokio::test]
async fn template_crud_roundtrips() {
    let (_dir, storage) = storage("templates-test.sqlite3").await;

    assert!(storage.list_templates().await.expect("list").is_empty());

    let created = storage
        .create_template(
            "  주간 스탠드업  ",
            "각 참석자별 진행 상황을 한 줄로 정리하세요.",
        )
        .await
        .expect("create");
    assert_eq!(created.name, "주간 스탠드업");
    assert_eq!(storage.list_templates().await.expect("list").len(), 1);

    storage
        .update_template(
            created.id,
            "주간 회고",
            "잘한 점과 개선점을 나누어 정리하세요.",
        )
        .await
        .expect("update");
    let stored = storage
        .get_template(created.id)
        .await
        .expect("get")
        .expect("template should exist");
    assert_eq!(stored.name, "주간 회고");
    assert_eq!(stored.content, "잘한 점과 개선점을 나누어 정리하세요.");

    storage.delete_template(created.id).await.expect("delete");
    assert!(storage.list_templates().await.expect("list").is_empty());
}

#[tokio::test]
async fn template_errors_are_typed() {
    let (_dir, storage) = storage("templates-error-test.sqlite3").await;

    assert!(matches!(
        storage
            .create_template("  ", "본문")
            .await
            .expect_err("blank name"),
        AppError::InvalidState(_)
    ));
    assert!(matches!(
        storage
            .update_template(Uuid::new_v4(), "이름", "본문")
            .await
            .expect_err("unknown template"),
        AppError::NotFound(_)
    ));
    assert!(matches!(
        storage
            .delete_template(Uuid::new_v4())
            .await
            .expect_err("unknown template"),
        AppError::NotFound(_)
    ));
}

#[test]
fn template_content_is_appended_to_the_minutes_system_prompt() {
    let base = minutes_system_prompt(None);
    assert!(!base.contains("사용자 템플릿"));

    // Blank templates are the same as no template at all.
    assert_eq!(minutes_system_prompt(Some("   ")), base);

    let with_template = minutes_system_prompt(Some("  섹션은 안건/결정/후속으로 나눈다.  "));
    assert!(with_template.starts_with(&base), "base rules must survive");
    assert!(with_template.contains("섹션은 안건/결정/후속으로 나눈다."));
}
