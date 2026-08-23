//! SQLite-backed storage for recordings, transcript segments, and minutes drafts.
//! Uses runtime-checked queries (`sqlx::query`/`query_as`), never the `query!` macro, so
//! `cargo check` never needs a live database connection at build time.

use crate::error::{AppError, AppResult};
use crate::models::{MinutesDraft, MinutesItem, Recording, RecordingStatus, TranscriptSegment};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::str::FromStr;
use uuid::Uuid;

pub struct Storage {
    pool: SqlitePool,
}

impl Storage {
    pub async fn connect(db_path: &Path) -> AppResult<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path.display()))?
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new().max_connections(5).connect_with(opts).await?;
        sqlx::migrate!("./migrations").run(&pool).await.map_err(|e| sqlx::Error::Migrate(Box::new(e)))?;
        Ok(Self { pool })
    }

    pub async fn insert_recording(&self, rec: &Recording) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO recordings (id, title, source_path, duration_ms, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(rec.id.to_string())
        .bind(&rec.title)
        .bind(&rec.source_path)
        .bind(rec.duration_ms)
        .bind(rec.status.as_str())
        .bind(rec.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_status(&self, id: Uuid, status: RecordingStatus) -> AppResult<()> {
        sqlx::query("UPDATE recordings SET status = ?1 WHERE id = ?2")
            .bind(status.as_str())
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_duration(&self, id: Uuid, duration_ms: i64) -> AppResult<()> {
        sqlx::query("UPDATE recordings SET duration_ms = ?1 WHERE id = ?2")
            .bind(duration_ms)
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_recordings(&self) -> AppResult<Vec<Recording>> {
        let rows = sqlx::query("SELECT id, title, source_path, duration_ms, status, created_at FROM recordings ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(row_to_recording).collect())
    }

    pub async fn get_recording(&self, id: Uuid) -> AppResult<Option<Recording>> {
        let row = sqlx::query("SELECT id, title, source_path, duration_ms, status, created_at FROM recordings WHERE id = ?1")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.as_ref().map(row_to_recording))
    }

    pub async fn insert_segments(&self, segments: &[TranscriptSegment]) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;
        for seg in segments {
            sqlx::query(
                "INSERT INTO transcript_segments (id, recording_id, start_ms, end_ms, speaker_label, text, is_final) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .bind(seg.id.to_string())
            .bind(seg.recording_id.to_string())
            .bind(seg.start_ms)
            .bind(seg.end_ms)
            .bind(&seg.speaker_label)
            .bind(&seg.text)
            .bind(seg.is_final as i64)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn list_segments(&self, recording_id: Uuid) -> AppResult<Vec<TranscriptSegment>> {
        let rows = sqlx::query(
            "SELECT id, recording_id, start_ms, end_ms, speaker_label, text, is_final FROM transcript_segments WHERE recording_id = ?1 ORDER BY start_ms ASC",
        )
        .bind(recording_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(row_to_segment).collect())
    }

    pub async fn save_minutes(&self, draft: &MinutesDraft) -> AppResult<()> {
        let decisions_json = serde_json::to_string(&draft.decisions).map_err(|error| {
            AppError::InvalidState(format!("failed to serialize minutes decisions: {error}"))
        })?;
        let action_items_json = serde_json::to_string(&draft.action_items).map_err(|error| {
            AppError::InvalidState(format!("failed to serialize minutes action items: {error}"))
        })?;
        sqlx::query(
            "INSERT INTO minutes_drafts (recording_id, summary, decisions_json, action_items_json, updated_at) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(recording_id) DO UPDATE SET summary = excluded.summary, decisions_json = excluded.decisions_json, action_items_json = excluded.action_items_json, updated_at = excluded.updated_at",
        )
        .bind(draft.recording_id.to_string())
        .bind(&draft.summary)
        .bind(decisions_json)
        .bind(action_items_json)
        .bind(draft.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_minutes(&self, recording_id: Uuid) -> AppResult<Option<MinutesDraft>> {
        let row = sqlx::query(
            "SELECT recording_id, summary, decisions_json, action_items_json, updated_at FROM minutes_drafts WHERE recording_id = ?1",
        )
        .bind(recording_id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };

        let stored_recording_id = Uuid::parse_str(row.get::<String, _>("recording_id").as_str())
            .map_err(|error| {
                AppError::InvalidState(format!("invalid stored minutes recording id: {error}"))
            })?;
        let decisions = serde_json::from_str::<Vec<MinutesItem>>(
            row.get::<String, _>("decisions_json").as_str(),
        )
        .map_err(|error| {
            AppError::InvalidState(format!("invalid stored minutes decisions: {error}"))
        })?;
        let action_items = serde_json::from_str::<Vec<MinutesItem>>(
            row.get::<String, _>("action_items_json").as_str(),
        )
        .map_err(|error| {
            AppError::InvalidState(format!("invalid stored minutes action items: {error}"))
        })?;
        let updated_at =
            chrono::DateTime::parse_from_rfc3339(row.get::<String, _>("updated_at").as_str())
                .map_err(|error| {
                    AppError::InvalidState(format!("invalid stored minutes timestamp: {error}"))
                })?
                .with_timezone(&chrono::Utc);

        Ok(Some(MinutesDraft {
            recording_id: stored_recording_id,
            summary: row.get("summary"),
            decisions,
            action_items,
            updated_at,
        }))
    }

    /// Reads a single app-level setting. Returns `None` when unset so callers apply their
    /// own defaults instead of the storage layer hard-coding policy.
    pub async fn get_setting(&self, key: &str) -> AppResult<Option<String>> {
        let row = sqlx::query("SELECT value FROM app_settings WHERE key = ?1")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|row| row.get::<String, _>("value")))
    }

    /// Writes a single app-level setting. Upsert semantics: rewriting an existing key
    /// replaces its value instead of violating the primary key.
    pub async fn set_setting(&self, key: &str, value: &str) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

fn row_to_recording(row: &sqlx::sqlite::SqliteRow) -> Recording {
    Recording {
        id: Uuid::parse_str(row.get::<String, _>("id").as_str()).unwrap_or_default(),
        title: row.get("title"),
        source_path: row.get("source_path"),
        duration_ms: row.get("duration_ms"),
        status: RecordingStatus::from_db_str(row.get::<String, _>("status").as_str()),
        created_at: chrono::DateTime::parse_from_rfc3339(row.get::<String, _>("created_at").as_str())
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now()),
    }
}

fn row_to_segment(row: &sqlx::sqlite::SqliteRow) -> TranscriptSegment {
    TranscriptSegment {
        id: Uuid::parse_str(row.get::<String, _>("id").as_str()).unwrap_or_default(),
        recording_id: Uuid::parse_str(row.get::<String, _>("recording_id").as_str()).unwrap_or_default(),
        start_ms: row.get("start_ms"),
        end_ms: row.get("end_ms"),
        speaker_label: row.get("speaker_label"),
        text: row.get("text"),
        is_final: row.get::<i64, _>("is_final") != 0,
    }
}
