-- User-defined folders for organizing recordings. A recording belongs to at most one
-- folder (nullable folder_id) — the simplest shape that satisfies the S1-S10 sidebar.
CREATE TABLE IF NOT EXISTS folders (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL
);

ALTER TABLE recordings ADD COLUMN folder_id TEXT REFERENCES folders(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_recordings_folder ON recordings(folder_id);
