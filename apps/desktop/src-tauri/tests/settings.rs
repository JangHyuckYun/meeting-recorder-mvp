//! Settings persistence contract: the app_settings KV table backs LLM provider selection.

use desktop_lib::storage::Storage;

#[tokio::test]
async fn app_settings_roundtrip_persists_and_defaults_to_absent() {
    let dir = tempfile::tempdir().expect("tempdir should create");
    let db_path = dir.path().join("settings-test.sqlite3");
    let storage = Storage::connect(&db_path)
        .await
        .expect("storage should connect and migrate");

    // Unset key reads back as absent so the command layer can apply its own default.
    let unset = storage
        .get_setting("llm_provider")
        .await
        .expect("get should succeed on fresh database");
    assert_eq!(unset, None);

    storage
        .set_setting("llm_provider", "codex_oauth")
        .await
        .expect("set should insert");
    let stored = storage
        .get_setting("llm_provider")
        .await
        .expect("get should return inserted value");
    assert_eq!(stored.as_deref(), Some("codex_oauth"));

    // Upsert semantics: rewriting the same key replaces instead of violating the PK.
    storage
        .set_setting("llm_provider", "claude_oauth")
        .await
        .expect("upsert should replace");
    let updated = storage
        .get_setting("llm_provider")
        .await
        .expect("get should return replaced value");
    assert_eq!(updated.as_deref(), Some("claude_oauth"));
}
