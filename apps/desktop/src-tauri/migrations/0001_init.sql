CREATE TABLE IF NOT EXISTS recordings (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    source_path TEXT NOT NULL,
    duration_ms INTEGER,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS transcript_segments (
    id TEXT PRIMARY KEY,
    recording_id TEXT NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
    start_ms INTEGER NOT NULL,
    end_ms INTEGER NOT NULL,
    speaker_label TEXT NOT NULL,
    text TEXT NOT NULL,
    is_final INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_segments_recording ON transcript_segments(recording_id);

CREATE TABLE IF NOT EXISTS minutes_drafts (
    recording_id TEXT PRIMARY KEY REFERENCES recordings(id) ON DELETE CASCADE,
    summary TEXT NOT NULL,
    decisions_json TEXT NOT NULL,
    action_items_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
