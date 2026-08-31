//! Settings persistence contract: the app_settings KV table backs LLM provider selection.

use desktop_lib::models::SttEngine;
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

#[tokio::test]
async fn stt_engine_defaults_and_roundtrips() {
    let dir = tempfile::tempdir().expect("tempdir should create");
    let db_path = dir.path().join("stt-engine-test.sqlite3");
    let storage = Storage::connect(&db_path)
        .await
        .expect("storage should connect and migrate");

    assert_eq!(
        storage
            .get_settings()
            .await
            .expect("settings should load")
            .stt_engine,
        SttEngine::SelfHosted
    );

    storage
        .set_setting("stt_engine", "elevenlabs")
        .await
        .expect("engine should persist");
    assert_eq!(
        storage
            .get_settings()
            .await
            .expect("settings should reload")
            .stt_engine,
        SttEngine::Elevenlabs
    );
}

#[tokio::test]
async fn elevenlabs_key_stored_raw_and_masked() {
    let dir = tempfile::tempdir().expect("tempdir should create");
    let db_path = dir.path().join("elevenlabs-key-test.sqlite3");
    let storage = Storage::connect(&db_path)
        .await
        .expect("storage should connect and migrate");
    let raw_key = "sk_live_ABCDEFGH1234";

    storage
        .set_setting("elevenlabs_api_key", raw_key)
        .await
        .expect("key should persist");
    assert_eq!(
        storage
            .elevenlabs_api_key()
            .await
            .expect("raw key should load")
            .as_deref(),
        Some(raw_key)
    );

    let masked = storage
        .get_settings()
        .await
        .expect("settings should load")
        .elevenlabs_api_key_masked
        .expect("masked key should be present");
    assert!(masked.contains('…'));
    assert_ne!(masked, raw_key);
}
