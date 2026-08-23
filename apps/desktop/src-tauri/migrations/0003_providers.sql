CREATE TABLE IF NOT EXISTS providers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    provider_type TEXT NOT NULL CHECK(provider_type IN ('openai', 'anthropic', 'openai_compatible')),
    base_url TEXT NOT NULL DEFAULT '',
    api_key_masked TEXT NOT NULL DEFAULT '',
    models_json TEXT NOT NULL DEFAULT '[]',
    is_active INTEGER NOT NULL DEFAULT 1,
    is_builtin INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS model_assignments (
    purpose TEXT PRIMARY KEY,
    provider_id TEXT NOT NULL,
    model_name TEXT NOT NULL,
    FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE CASCADE
);

-- Seed built-in OAuth providers on fresh install. They are marked is_builtin=1 so the
-- frontend shows them as read-only registry entries with their special auth flows.
INSERT OR IGNORE INTO providers (id, name, provider_type, base_url, api_key_masked, models_json, is_active, is_builtin, created_at)
VALUES
  ('00000000-0000-0000-0000-000000000001', 'ChatGPT 구독 (Codex OAuth)', 'openai', '', '', '["gpt-4o","gpt-4.1-mini","gpt-4.1-nano"]', 1, 1, '2026-01-01T00:00:00Z'),
  ('00000000-0000-0000-0000-000000000002', 'Claude 구독 (Claude OAuth)', 'anthropic', '', '', '["claude-sonnet-4-20250514","claude-sonnet-4","claude-3.5-haiku"]', 1, 1, '2026-01-01T00:00:00Z');

-- Seed default model assignments pointing to codex_oauth for minutes generation/edit.
INSERT OR IGNORE INTO model_assignments (purpose, provider_id, model_name)
VALUES
  ('minutes_generation', '00000000-0000-0000-0000-000000000001', 'gpt-4.1-mini'),
  ('minutes_edit', '00000000-0000-0000-0000-000000000001', 'gpt-4.1-mini');