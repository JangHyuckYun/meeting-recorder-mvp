-- App-level key/value settings (currently: LLM provider selection).
-- Values are opaque strings interpreted by the command layer, never by SQL.
CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
