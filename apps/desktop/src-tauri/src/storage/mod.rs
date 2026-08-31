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
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await?;
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| sqlx::Error::Migrate(Box::new(e)))?;
        Ok(Self { pool })
    }

    pub async fn insert_recording(&self, rec: &Recording) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO recordings (id, title, source_path, duration_ms, status, created_at, folder_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(rec.id.to_string())
        .bind(&rec.title)
        .bind(&rec.source_path)
        .bind(rec.duration_ms)
        .bind(rec.status.as_str())
        .bind(rec.created_at.to_rfc3339())
        .bind(rec.folder_id.map(|id| id.to_string()))
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
        let rows = sqlx::query("SELECT id, title, source_path, duration_ms, status, created_at, folder_id FROM recordings ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(row_to_recording).collect())
    }

    pub async fn get_recording(&self, id: Uuid) -> AppResult<Option<Recording>> {
        let row = sqlx::query("SELECT id, title, source_path, duration_ms, status, created_at, folder_id FROM recordings WHERE id = ?1")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.as_ref().map(row_to_recording))
    }

    /// Deletes a recording and everything hanging off it. SQLite foreign keys are not
    /// enabled on this pool, so the children are removed explicitly rather than by cascade.
    /// Returns `NotFound` when the id does not exist so the command layer can report it.
    pub async fn delete_recording(&self, id: Uuid) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;
        let deleted = sqlx::query("DELETE FROM recordings WHERE id = ?1")
            .bind(id.to_string())
            .execute(&mut *tx)
            .await?
            .rows_affected();
        if deleted == 0 {
            return Err(AppError::NotFound(format!("recording {id} not found")));
        }
        for table in ["transcript_segments", "minutes_drafts"] {
            sqlx::query(&format!("DELETE FROM {table} WHERE recording_id = ?1"))
                .bind(id.to_string())
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("DELETE FROM app_settings WHERE key = ?1")
            .bind(speaker_names_key(id))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Folders
    // ------------------------------------------------------------------

    pub async fn list_folders(&self) -> AppResult<Vec<crate::models::Folder>> {
        let rows = sqlx::query("SELECT id, name, created_at FROM folders ORDER BY created_at ASC")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(row_to_folder).collect())
    }

    pub async fn create_folder(&self, name: &str) -> AppResult<crate::models::Folder> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::InvalidState(
                "folder name cannot be empty".to_string(),
            ));
        }
        let folder = crate::models::Folder {
            id: Uuid::new_v4(),
            name: name.to_string(),
            created_at: chrono::Utc::now(),
        };
        sqlx::query("INSERT INTO folders (id, name, created_at) VALUES (?1, ?2, ?3)")
            .bind(folder.id.to_string())
            .bind(&folder.name)
            .bind(folder.created_at.to_rfc3339())
            .execute(&self.pool)
            .await?;
        Ok(folder)
    }

    /// Deletes a folder; its recordings survive and fall back to the unfiled list.
    pub async fn delete_folder(&self, id: Uuid) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;
        let deleted = sqlx::query("DELETE FROM folders WHERE id = ?1")
            .bind(id.to_string())
            .execute(&mut *tx)
            .await?
            .rows_affected();
        if deleted == 0 {
            return Err(AppError::NotFound(format!("folder {id} not found")));
        }
        sqlx::query("UPDATE recordings SET folder_id = NULL WHERE folder_id = ?1")
            .bind(id.to_string())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Files a recording under a folder, or unfiles it when `folder_id` is `None`.
    pub async fn assign_recording_folder(
        &self,
        recording_id: Uuid,
        folder_id: Option<Uuid>,
    ) -> AppResult<()> {
        let affected = sqlx::query("UPDATE recordings SET folder_id = ?1 WHERE id = ?2")
            .bind(folder_id.map(|id| id.to_string()))
            .bind(recording_id.to_string())
            .execute(&self.pool)
            .await?
            .rows_affected();
        if affected == 0 {
            return Err(AppError::NotFound(format!(
                "recording {recording_id} not found"
            )));
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Minutes templates
    // ------------------------------------------------------------------

    pub async fn list_templates(&self) -> AppResult<Vec<crate::models::Template>> {
        let rows = sqlx::query(
            "SELECT id, name, content, created_at FROM templates ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(row_to_template).collect())
    }

    pub async fn get_template(&self, id: Uuid) -> AppResult<Option<crate::models::Template>> {
        let row = sqlx::query("SELECT id, name, content, created_at FROM templates WHERE id = ?1")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.as_ref().map(row_to_template))
    }

    pub async fn create_template(
        &self,
        name: &str,
        content: &str,
    ) -> AppResult<crate::models::Template> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::InvalidState(
                "template name cannot be empty".to_string(),
            ));
        }
        let template = crate::models::Template {
            id: Uuid::new_v4(),
            name: name.to_string(),
            content: content.to_string(),
            created_at: chrono::Utc::now(),
        };
        sqlx::query(
            "INSERT INTO templates (id, name, content, created_at) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(template.id.to_string())
        .bind(&template.name)
        .bind(&template.content)
        .bind(template.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(template)
    }

    pub async fn update_template(&self, id: Uuid, name: &str, content: &str) -> AppResult<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::InvalidState(
                "template name cannot be empty".to_string(),
            ));
        }
        let affected = sqlx::query("UPDATE templates SET name = ?2, content = ?3 WHERE id = ?1")
            .bind(id.to_string())
            .bind(name)
            .bind(content)
            .execute(&self.pool)
            .await?
            .rows_affected();
        if affected == 0 {
            return Err(AppError::NotFound(format!("template {id} not found")));
        }
        Ok(())
    }

    pub async fn delete_template(&self, id: Uuid) -> AppResult<()> {
        let deleted = sqlx::query("DELETE FROM templates WHERE id = ?1")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?
            .rows_affected();
        if deleted == 0 {
            return Err(AppError::NotFound(format!("template {id} not found")));
        }
        Ok(())
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

    /// Returns all app settings as a single AppSettings struct.
    pub async fn get_settings(&self) -> AppResult<crate::models::AppSettings> {
        let llm_provider = self
            .get_setting("llm_provider")
            .await?
            .as_deref()
            .and_then(crate::models::LlmProvider::from_db_str)
            .unwrap_or_default();
        let stt_server_url = self.get_setting("stt_server_url").await?;
        let stt_engine = self
            .get_setting("stt_engine")
            .await?
            .as_deref()
            .and_then(crate::models::SttEngine::from_db_str)
            .unwrap_or_default();
        let elevenlabs_api_key_masked = self
            .elevenlabs_api_key()
            .await?
            .map(|key| mask_api_key(&key));
        Ok(crate::models::AppSettings {
            llm_provider,
            stt_server_url,
            stt_engine,
            elevenlabs_api_key_masked,
        })
    }

    /// Returns the raw ElevenLabs key for authenticated server-side requests.
    pub async fn elevenlabs_api_key(&self) -> AppResult<Option<String>> {
        self.get_setting("elevenlabs_api_key").await
    }

    /// Domain glossary (keyterms) biasing STT. Stored as a JSON array in the settings KV;
    /// unset or corrupt values read back as an empty glossary rather than failing.
    pub async fn get_glossary(&self) -> AppResult<Vec<String>> {
        Ok(self
            .get_setting("glossary")
            .await?
            .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok())
            .unwrap_or_default())
    }

    /// Per-recording speaker display names, keyed by the diarization label
    /// (e.g. "화자 1" → "김민지"). Stored as one JSON object per recording in the settings KV
    /// rather than its own table — the map is tiny and always read whole.
    pub async fn get_speaker_names(
        &self,
        recording_id: Uuid,
    ) -> AppResult<std::collections::BTreeMap<String, String>> {
        Ok(self
            .get_setting(&speaker_names_key(recording_id))
            .await?
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default())
    }

    /// Sets one speaker's display name. A blank name clears the override.
    pub async fn set_speaker_name(
        &self,
        recording_id: Uuid,
        speaker_key: &str,
        name: &str,
    ) -> AppResult<()> {
        let speaker_key = speaker_key.trim();
        if speaker_key.is_empty() {
            return Err(AppError::InvalidState(
                "speaker key cannot be empty".to_string(),
            ));
        }
        let mut names = self.get_speaker_names(recording_id).await?;
        match name.trim() {
            "" => {
                names.remove(speaker_key);
            }
            name => {
                names.insert(speaker_key.to_string(), name.to_string());
            }
        }
        let json = serde_json::to_string(&names).map_err(|error| {
            AppError::InvalidState(format!("failed to serialize speaker names: {error}"))
        })?;
        self.set_setting(&speaker_names_key(recording_id), &json)
            .await
    }

    pub async fn set_glossary(&self, terms: &[String]) -> AppResult<()> {
        let cleaned: Vec<&str> = terms
            .iter()
            .map(|term| term.trim())
            .filter(|term| !term.is_empty())
            .collect();
        let json = serde_json::to_string(&cleaned).map_err(|error| {
            AppError::InvalidState(format!("failed to serialize glossary: {error}"))
        })?;
        self.set_setting("glossary", &json).await
    }

    // ------------------------------------------------------------------
    // Provider registry
    // ------------------------------------------------------------------

    pub async fn list_providers(&self) -> AppResult<Vec<crate::models::ProviderSummary>> {
        let rows = sqlx::query(
            "SELECT id, name, provider_type, base_url, api_key_masked, models_json, is_active, is_builtin, created_at FROM providers ORDER BY is_builtin DESC, created_at ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut providers: Vec<crate::models::ProviderSummary> =
            rows.iter().map(row_to_provider).collect();
        // Mask stored API keys before returning to frontend
        for provider in &mut providers {
            if !provider.api_key_masked.is_empty() {
                provider.api_key_masked = mask_api_key(&provider.api_key_masked);
            }
        }
        Ok(providers)
    }

    pub async fn add_provider(&self, input: &crate::models::ProviderInput) -> AppResult<Uuid> {
        let id = Uuid::new_v4();
        let provider_type = crate::models::ProviderType::from_db_str(&input.provider_type)
            .ok_or_else(|| {
                AppError::InvalidState(format!("unknown provider type: {}", input.provider_type))
            })?;
        sqlx::query(
            "INSERT INTO providers (id, name, provider_type, base_url, api_key_masked, models_json, is_active, is_builtin, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, 0, ?7)",
        )
        .bind(id.to_string())
        .bind(&input.name)
        .bind(provider_type.as_str())
        .bind(&input.base_url)
        .bind(&input.api_key)
        .bind(&input.models_json)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn update_provider(&self, input: &crate::models::ProviderInput) -> AppResult<()> {
        let id = input
            .id
            .as_deref()
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| AppError::InvalidState("provider id required for update".to_string()))?;
        let provider_type = crate::models::ProviderType::from_db_str(&input.provider_type)
            .ok_or_else(|| {
                AppError::InvalidState(format!("unknown provider type: {}", input.provider_type))
            })?;
        let api_key = if input.api_key.is_empty() {
            // keep existing key if no new key provided
            let row = sqlx::query("SELECT api_key_masked FROM providers WHERE id = ?1")
                .bind(id.to_string())
                .fetch_optional(&self.pool)
                .await?;
            row.map(|r| r.get::<String, _>("api_key_masked"))
                .unwrap_or_default()
        } else {
            input.api_key.clone()
        };
        sqlx::query(
            "UPDATE providers SET name = ?2, provider_type = ?3, base_url = ?4, api_key_masked = ?5, models_json = ?6 WHERE id = ?1 AND is_builtin = 0",
        )
        .bind(id.to_string())
        .bind(&input.name)
        .bind(provider_type.as_str())
        .bind(&input.base_url)
        .bind(&api_key)
        .bind(&input.models_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_provider(&self, id: Uuid) -> AppResult<()> {
        // cascade-delete associated model_assignments first
        sqlx::query("DELETE FROM model_assignments WHERE provider_id = ?1")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM providers WHERE id = ?1 AND is_builtin = 0")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_assigned_provider_model(
        &self,
        purpose: &str,
    ) -> AppResult<Option<(Uuid, String)>> {
        let row =
            sqlx::query("SELECT provider_id, model_name FROM model_assignments WHERE purpose = ?1")
                .bind(purpose)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| {
            let pid =
                Uuid::parse_str(r.get::<String, _>("provider_id").as_str()).unwrap_or_default();
            (pid, r.get::<String, _>("model_name"))
        }))
    }

    pub async fn get_provider(
        &self,
        id: Uuid,
    ) -> AppResult<Option<crate::models::ProviderSummary>> {
        let row = sqlx::query("SELECT id, name, provider_type, base_url, api_key_masked, models_json, is_active, is_builtin, created_at FROM providers WHERE id = ?1")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.as_ref().map(row_to_provider))
    }

    pub async fn set_model_assignment(
        &self,
        purpose: &str,
        provider_id: &str,
        model_name: &str,
    ) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO model_assignments (purpose, provider_id, model_name) VALUES (?1, ?2, ?3) ON CONFLICT(purpose) DO UPDATE SET provider_id = excluded.provider_id, model_name = excluded.model_name",
        )
        .bind(purpose)
        .bind(provider_id)
        .bind(model_name)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_model_assignments(&self) -> AppResult<Vec<crate::models::ModelAssignment>> {
        let rows = sqlx::query("SELECT purpose, provider_id, model_name FROM model_assignments")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(row_to_assignment).collect())
    }
}

fn speaker_names_key(recording_id: Uuid) -> String {
    format!("speaker_names:{recording_id}")
}

fn row_to_recording(row: &sqlx::sqlite::SqliteRow) -> Recording {
    Recording {
        id: Uuid::parse_str(row.get::<String, _>("id").as_str()).unwrap_or_default(),
        title: row.get("title"),
        source_path: row.get("source_path"),
        duration_ms: row.get("duration_ms"),
        status: RecordingStatus::from_db_str(row.get::<String, _>("status").as_str()),
        created_at: chrono::DateTime::parse_from_rfc3339(
            row.get::<String, _>("created_at").as_str(),
        )
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now()),
        folder_id: row
            .get::<Option<String>, _>("folder_id")
            .and_then(|id| Uuid::parse_str(&id).ok()),
    }
}

fn row_to_folder(row: &sqlx::sqlite::SqliteRow) -> crate::models::Folder {
    crate::models::Folder {
        id: Uuid::parse_str(row.get::<String, _>("id").as_str()).unwrap_or_default(),
        name: row.get("name"),
        created_at: chrono::DateTime::parse_from_rfc3339(
            row.get::<String, _>("created_at").as_str(),
        )
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now()),
    }
}

fn row_to_template(row: &sqlx::sqlite::SqliteRow) -> crate::models::Template {
    crate::models::Template {
        id: Uuid::parse_str(row.get::<String, _>("id").as_str()).unwrap_or_default(),
        name: row.get("name"),
        content: row.get("content"),
        created_at: chrono::DateTime::parse_from_rfc3339(
            row.get::<String, _>("created_at").as_str(),
        )
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now()),
    }
}

fn row_to_segment(row: &sqlx::sqlite::SqliteRow) -> TranscriptSegment {
    TranscriptSegment {
        id: Uuid::parse_str(row.get::<String, _>("id").as_str()).unwrap_or_default(),
        recording_id: Uuid::parse_str(row.get::<String, _>("recording_id").as_str())
            .unwrap_or_default(),
        start_ms: row.get("start_ms"),
        end_ms: row.get("end_ms"),
        speaker_label: row.get("speaker_label"),
        text: row.get("text"),
        is_final: row.get::<i64, _>("is_final") != 0,
    }
}

fn row_to_provider(row: &sqlx::sqlite::SqliteRow) -> crate::models::ProviderSummary {
    use crate::models::*;
    let models_json: String = row.get("models_json");
    let models: Vec<String> = serde_json::from_str(&models_json).unwrap_or_default();
    ProviderSummary {
        id: Uuid::parse_str(row.get::<String, _>("id").as_str()).unwrap_or_default(),
        name: row.get("name"),
        provider_type: row.get::<String, _>("provider_type"),
        base_url: row.get("base_url"),
        api_key_masked: row.get("api_key_masked"),
        models,
        is_active: row.get::<i64, _>("is_active") != 0,
        is_builtin: row.get::<i64, _>("is_builtin") != 0,
    }
}

fn row_to_assignment(row: &sqlx::sqlite::SqliteRow) -> crate::models::ModelAssignment {
    use crate::models::*;
    ModelAssignment {
        purpose: ModelPurpose::from_db_str(row.get::<String, _>("purpose").as_str())
            .unwrap_or_default(),
        provider_id: Uuid::parse_str(row.get::<String, _>("provider_id").as_str())
            .unwrap_or_default(),
        model_name: row.get("model_name"),
    }
}

/// Masks an API key for storage, showing only the first 8 characters. The actual key is
/// never stored; only the masked form is persisted so a DB leak does not expose secrets.
fn mask_api_key(key: &str) -> String {
    if key.len() <= 8 || !key.contains(|c: char| c.is_alphanumeric()) {
        return key.to_string();
    }
    let visible: String = key.chars().take(8).collect();
    format!("{visible}…{rest}", rest = key.len().saturating_sub(8))
}
