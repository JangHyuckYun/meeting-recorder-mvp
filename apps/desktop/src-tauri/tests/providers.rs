//! Provider registry + model assignment contract: the providers and model_assignments tables
//! back the new dual-panel settings UI.

use desktop_lib::models::{ModelPurpose, ProviderInput};
use desktop_lib::storage::Storage;

/// Before any provider is added, the list should return built-in seed providers.
#[tokio::test]
async fn list_providers_returns_seeded_builtins() {
    let dir = tempfile::tempdir().expect("tempdir should create");
    let db_path = dir.path().join("providers-test-1.sqlite3");
    let storage = Storage::connect(&db_path)
        .await
        .expect("storage should connect and migrate");

    let providers = storage
        .list_providers()
        .await
        .expect("list_providers should succeed");
    assert!(
        providers.len() >= 2,
        "expected at least 2 built-in providers, got {}",
        providers.len()
    );

    let codex = providers.iter().find(|p| p.is_builtin && p.name.contains("Codex"));
    assert!(codex.is_some(), "expected built-in Codex OAuth provider");
    let claude = providers.iter().find(|p| p.is_builtin && p.name.contains("Claude"));
    assert!(claude.is_some(), "expected built-in Claude OAuth provider");
}

/// Adding a user provider round-trips with a generated id and correct fields.
#[tokio::test]
async fn add_and_list_user_provider_roundtrip() {
    let dir = tempfile::tempdir().expect("tempdir should create");
    let db_path = dir.path().join("providers-test-2.sqlite3");
    let storage = Storage::connect(&db_path)
        .await
        .expect("storage should connect and migrate");

    let id = storage
        .add_provider(&ProviderInput {
            id: None,
            name: "My OpenAI Key".into(),
            provider_type: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            api_key: "sk-proj-my-real-key".into(),
            models_json: r#"["gpt-4o","gpt-4.1-mini"]"#.into(),
        })
        .await
        .expect("add_provider should succeed");

    let providers = storage
        .list_providers()
        .await
        .expect("list_providers should succeed");
    let added = providers
        .iter()
        .find(|p| p.id == id)
        .expect("added provider should appear in list");

    assert_eq!(added.name, "My OpenAI Key");
    assert_eq!(added.provider_type, "openai");
    assert_eq!(added.base_url, "https://api.openai.com/v1");
    // api_key should be masked when listed
    assert!(added.api_key_masked.starts_with("sk-proj-"), "key should be masked: {}", added.api_key_masked);
    assert!(added.api_key_masked.contains("…"), "key should contain ellipsis: {}", added.api_key_masked);
    assert!(!added.is_builtin);
    assert!(added.is_active);
}

/// Deleting a user provider also cascades its model assignments.
#[tokio::test]
async fn delete_user_provider_cascades_assignments() {
    let dir = tempfile::tempdir().expect("tempdir should create");
    let db_path = dir.path().join("providers-test-3.sqlite3");
    let storage = Storage::connect(&db_path)
        .await
        .expect("storage should connect and migrate");

    let id = storage
        .add_provider(&ProviderInput {
            id: None,
            name: "Temp Provider".into(),
            provider_type: "openai_compatible".into(),
            base_url: "http://localhost:8000/v1".into(),
            api_key: "".into(),
            models_json: r#"["qwen-72b"]"#.into(),
        })
        .await
        .expect("add_provider should succeed");

    // Set an assignment pointing to this provider
    storage
        .set_model_assignment("minutes_generation", &id.to_string(), "qwen-72b", None, false)
        .await
        .expect("set_model_assignment should succeed");

    // Delete the provider
    storage
        .delete_provider(id)
        .await
        .expect("delete_provider should succeed");

    let providers = storage
        .list_providers()
        .await
        .expect("list_providers should succeed");
    assert!(
        providers.iter().all(|p| p.id != id),
        "deleted provider should not appear in list"
    );

    // The assignment should be gone too
    let assigned = storage
        .get_assigned_provider_model("minutes_generation")
        .await
        .expect("get_assigned_provider_model should succeed");
    assert!(assigned.is_none(), "assignment should cascade-delete");
}

/// Set and list model assignments round-trip correctly.
#[tokio::test]
async fn model_assignment_set_and_list() {
    let dir = tempfile::tempdir().expect("tempdir should create");
    let db_path = dir.path().join("providers-test-4.sqlite3");
    let storage = Storage::connect(&db_path)
        .await
        .expect("storage should connect and migrate");

    // Built-in providers exist; use the codex_oauth id
    let providers = storage
        .list_providers()
        .await
        .expect("list_providers should succeed");
    let codex = providers
        .iter()
        .find(|p| p.is_builtin && p.name.contains("Codex"))
        .expect("codex builtin provider should exist");

    storage
        .set_model_assignment(
            "minutes_generation",
            &codex.id.to_string(),
            "gpt-4.1-mini",
            Some("high"),
            true,
        )
        .await
        .expect("set_model_assignment should succeed");

    let assignments = storage
        .list_model_assignments()
        .await
        .expect("list_model_assignments should succeed");
    let gen = assignments
        .iter()
        .find(|a| a.purpose == ModelPurpose::MinutesGeneration)
        .expect("minutes_generation assignment should exist");

    assert_eq!(gen.provider_id, codex.id);
    assert_eq!(gen.model_name, "gpt-4.1-mini");
    assert_eq!((gen.reasoning_effort.as_deref(), gen.fast), (Some("high"), true));
}

/// Built-in providers cannot be deleted.
#[tokio::test]
async fn cannot_delete_builtin_provider() {
    let dir = tempfile::tempdir().expect("tempdir should create");
    let db_path = dir.path().join("providers-test-5.sqlite3");
    let storage = Storage::connect(&db_path)
        .await
        .expect("storage should connect and migrate");

    let providers = storage
        .list_providers()
        .await
        .expect("list_providers should succeed");
    let builtin = providers
        .iter()
        .find(|p| p.is_builtin)
        .expect("a builtin provider should exist");

    // Deleting a builtin is a no-op (no error, but should not remove it)
    storage
        .delete_provider(builtin.id)
        .await
        .expect("delete_provider on builtin should not error");

    let providers_after = storage
        .list_providers()
        .await
        .expect("list_providers should succeed");
    assert!(
        providers_after.iter().any(|p| p.id == builtin.id),
        "builtin provider should survive deletion"
    );
}
