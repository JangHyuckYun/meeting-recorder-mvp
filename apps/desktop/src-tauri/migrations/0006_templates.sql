-- User-authored minutes templates. `content` is free-form Korean instruction text that is
-- appended to the minutes system prompt when a generation names the template.
CREATE TABLE IF NOT EXISTS templates (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL
);
