//! Claude subscription-backed structured generation using Claude Code's OAuth credentials.
//!
//! The installed Claude CLI stores OAuth tokens in `~/.claude/.credentials.json` under
//! `claudeAiOauth`, refreshes them at `https://platform.claude.com/v1/oauth/token`, and calls
//! `https://api.anthropic.com/v1/messages` with Bearer auth plus the Claude Code identity
//! betas. This module mirrors that contract so meeting minutes can run on a Claude
//! subscription without requiring a separate API key.

use crate::error::{AppError, AppResult};
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DEFAULT_ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com";
const DEFAULT_CLAUDE_MODEL: &str = "claude-sonnet-4-5";
const ANTHROPIC_TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";
const ANTHROPIC_OAUTH_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const ANTHROPIC_API_VERSION: &str = "2023-06-01";
const ANTHROPIC_BETAS: &str = "claude-code-20250219,oauth-2025-04-20";
const TOKEN_REFRESH_MARGIN_MS: u64 = 60_000;

#[derive(Debug, Clone)]
pub(super) struct ClaudeCredentials {
    pub access_token: String,
    pub refresh_token: String,
    /// Access-token expiry in Unix epoch milliseconds, when the credential file records it.
    pub expires_at_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct RefreshResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
}

pub(super) async fn request_structured_json(
    system_prompt: &str,
    user_prompt: &str,
    schema: &Value,
) -> AppResult<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()?;
    let credentials_path = credentials_path()?;
    let mut credentials_file = read_credentials_file(&credentials_path)?;
    let credentials = parse_credentials(&credentials_file)?;

    let now = now_ms()?;
    let needs_refresh = credentials
        .expires_at_ms
        .is_some_and(|expires_at| expires_at <= now.saturating_add(TOKEN_REFRESH_MARGIN_MS));
    let mut credentials = credentials;
    if needs_refresh {
        credentials = refresh_credentials(&client, &credentials).await?;
        persist_credentials(&credentials_path, &mut credentials_file, &credentials)?;
    }

    let base_url = std::env::var("MINUTES_CLAUDE_BASE_URL")
        .unwrap_or_else(|_| DEFAULT_ANTHROPIC_BASE_URL.to_string());
    let model = std::env::var("MINUTES_CLAUDE_MODEL")
        .unwrap_or_else(|_| DEFAULT_CLAUDE_MODEL.to_string());
    let endpoint = format!("{}/v1/messages", base_url.trim_end_matches('/'));
    let schema_json = serde_json::to_string(schema).map_err(|error| {
        AppError::InvalidState(format!(
            "failed to serialize Claude OAuth LLM JSON schema: {error}"
        ))
    })?;
    let instructions = format!(
        "{system_prompt}\n\nReturn exactly one valid JSON object and no other text. Do not use Markdown fences. The object must conform to this JSON Schema:\n{schema_json}"
    );

    let response = client
        .post(endpoint)
        .bearer_auth(&credentials.access_token)
        .header("anthropic-version", ANTHROPIC_API_VERSION)
        .header("anthropic-beta", ANTHROPIC_BETAS)
        .header("accept", "application/json")
        .json(&json!({
            "model": model,
            "max_tokens": 4096,
            "system": instructions,
            "messages": [{"role": "user", "content": user_prompt}]
        }))
        .send()
        .await?;
    let status = response.status();
    let response_body = response.text().await?;
    if !status.is_success() {
        return Err(claude_request_error(status, &response_body));
    }

    let text = response_text_from_messages(&response_body)?;
    normalize_json_output(&text)
}

fn credentials_path() -> AppResult<PathBuf> {
    if let Ok(path) = std::env::var("MINUTES_CLAUDE_CREDENTIALS_PATH") {
        if !path.trim().is_empty() {
            return Ok(PathBuf::from(path));
        }
    }

    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or_else(|| {
            AppError::InvalidState(
                "cannot locate Claude OAuth credentials because HOME is not set".to_string(),
            )
        })?;
    Ok(PathBuf::from(home).join(".claude/.credentials.json"))
}

fn read_credentials_file(path: &Path) -> AppResult<Value> {
    let contents = fs::read_to_string(path).map_err(|error| {
        AppError::InvalidState(format!(
            "cannot read Claude OAuth credentials at {}: {error}; run `claude login`",
            path.display()
        ))
    })?;
    serde_json::from_str(&contents).map_err(|error| {
        AppError::InvalidState(format!(
            "Claude OAuth credentials at {} are invalid JSON: {error}; run `claude login`",
            path.display()
        ))
    })
}

/// Validates the parsed credential file into usable credentials. Kept separate from file IO
/// so unit tests can pin the accepted shape without touching the real credential store.
pub(super) fn parse_credentials(credentials_file: &Value) -> AppResult<ClaudeCredentials> {
    let invalid_shape = |detail: String| {
        AppError::InvalidState(format!(
            "Claude OAuth credentials have an unsupported shape ({detail}); run `claude login`"
        ))
    };
    let oauth = credentials_file
        .get("claudeAiOauth")
        .ok_or_else(|| invalid_shape("missing object `claudeAiOauth`".to_string()))?;

    let read_field = |key: &str| -> AppResult<String> {
        oauth
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned)
            .ok_or_else(|| invalid_shape(format!("missing `{key}`")))
    };

    let expires_at_ms = match oauth.get("expiresAt") {
        None | Some(Value::Null) => None,
        Some(value) => {
            let raw = value.as_i64().ok_or_else(|| {
                invalid_shape("`expiresAt` must be a Unix epoch in milliseconds".to_string())
            })?;
            Some(u64::try_from(raw).map_err(|_| {
                invalid_shape("`expiresAt` predates the Unix epoch".to_string())
            })?)
        }
    };

    Ok(ClaudeCredentials {
        access_token: read_field("accessToken")?,
        refresh_token: read_field("refreshToken")?,
        expires_at_ms,
    })
}

async fn refresh_credentials(
    client: &reqwest::Client,
    current: &ClaudeCredentials,
) -> AppResult<ClaudeCredentials> {
    let response = client
        .post(ANTHROPIC_TOKEN_URL)
        .json(&json!({
            "grant_type": "refresh_token",
            "client_id": ANTHROPIC_OAUTH_CLIENT_ID,
            "refresh_token": current.refresh_token,
        }))
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        let detail = backend_error_message(&body).unwrap_or_else(|| {
            status
                .canonical_reason()
                .unwrap_or("unknown error")
                .to_string()
        });
        return Err(AppError::InvalidState(format!(
            "Claude OAuth token refresh failed with HTTP {status}: {detail}; run `claude login` and retry"
        )));
    }
    let refreshed: RefreshResponse = serde_json::from_str(&body).map_err(|error| {
        AppError::InvalidState(format!(
            "Claude OAuth token refresh returned invalid JSON: {error}; run `claude login` and retry"
        ))
    })?;
    if refreshed.access_token.trim().is_empty() {
        return Err(AppError::InvalidState(
            "Claude OAuth token refresh returned an empty access token; run `claude login` and retry"
                .to_string(),
        ));
    }

    // Anthropic may rotate the refresh token or keep the existing one; both are valid
    // responses, so fall back to the current refresh token instead of inventing a failure.
    let expires_at_ms = match refreshed.expires_in {
        Some(seconds) => {
            let now = now_ms()?;
            Some(now.saturating_add(seconds.saturating_mul(1_000)))
        }
        None => None,
    };
    Ok(ClaudeCredentials {
        access_token: refreshed.access_token,
        refresh_token: refreshed
            .refresh_token
            .filter(|token| !token.trim().is_empty())
            .unwrap_or_else(|| current.refresh_token.clone()),
        expires_at_ms,
    })
}

/// Atomically persists rotated credentials back to the credential file: written to a
/// temporary sibling first (mode 600 on unix), then renamed over the original so a crash
/// mid-write can never destroy the user's only copy of the tokens.
fn persist_credentials(
    path: &Path,
    credentials_file: &mut Value,
    credentials: &ClaudeCredentials,
) -> AppResult<()> {
    let invalid_shape = |detail: &str| {
        AppError::InvalidState(format!(
            "Claude OAuth credentials at {} have an unsupported shape ({detail}); refusing to overwrite them",
            path.display()
        ))
    };
    let root = credentials_file
        .as_object_mut()
        .ok_or_else(|| invalid_shape("root is not an object"))?;
    let oauth = root
        .get_mut("claudeAiOauth")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid_shape("missing object `claudeAiOauth`"))?;
    oauth.insert(
        "accessToken".to_string(),
        Value::String(credentials.access_token.clone()),
    );
    oauth.insert(
        "refreshToken".to_string(),
        Value::String(credentials.refresh_token.clone()),
    );
    if let Some(expires_at_ms) = credentials.expires_at_ms {
        let expires_at = i64::try_from(expires_at_ms).map_err(|_| {
            invalid_shape("refreshed expiry does not fit the stored millisecond epoch")
        })?;
        oauth.insert("expiresAt".to_string(), Value::from(expires_at));
    }

    let parent = path.parent().ok_or_else(|| {
        AppError::InvalidState(format!(
            "Claude OAuth credential path {} has no parent directory",
            path.display()
        ))
    })?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(".credentials.json");
    let temporary_path = parent.join(format!(
        ".{file_name}.minutes-{}-{}",
        std::process::id(),
        now_ms()?
    ));
    let result = (|| -> AppResult<()> {
        let mut temporary = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary_path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            temporary.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        serde_json::to_writer_pretty(&mut temporary, credentials_file).map_err(|error| {
            AppError::InvalidState(format!(
                "failed to serialize refreshed Claude OAuth credentials: {error}"
            ))
        })?;
        temporary.write_all(b"\n")?;
        temporary.sync_all()?;
        drop(temporary);
        fs::rename(&temporary_path, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result.map_err(|error| {
        AppError::InvalidState(format!(
            "refreshed the Claude OAuth token but could not safely update {}: {error}",
            path.display()
        ))
    })
}

/// Extracts the concatenated assistant text blocks from a non-streaming Messages API
/// response. Unit-tested against captured response shapes.
pub(super) fn response_text_from_messages(body: &str) -> AppResult<String> {
    let parsed: Value = serde_json::from_str(body).map_err(|error| {
        AppError::InvalidState(format!(
            "Claude OAuth LLM returned malformed JSON: {error}"
        ))
    })?;
    let blocks = parsed.get("content").and_then(Value::as_array).ok_or_else(|| {
        AppError::InvalidState(
            "Claude OAuth LLM response has no `content` array".to_string(),
        )
    })?;
    let mut text = String::new();
    for block in blocks {
        if block.get("type").and_then(Value::as_str) == Some("text") {
            if let Some(part) = block.get("text").and_then(Value::as_str) {
                text.push_str(part);
            }
        }
    }
    if text.trim().is_empty() {
        return Err(AppError::InvalidState(
            "Claude OAuth LLM returned no response text".to_string(),
        ));
    }
    Ok(text)
}

fn normalize_json_output(text: &str) -> AppResult<String> {
    let trimmed = text.trim();
    let candidate = if let Some(fenced) = trimmed.strip_prefix("```") {
        let after_language = fenced.split_once('\n').map_or(fenced, |(_, body)| body);
        after_language
            .strip_suffix("```")
            .map(str::trim)
            .unwrap_or(after_language.trim())
    } else {
        trimmed
    };
    let value: Value = serde_json::from_str(candidate).map_err(|error| {
        AppError::InvalidState(format!(
            "Claude OAuth LLM returned malformed JSON instead of the requested schema: {error}"
        ))
    })?;
    if !value.is_object() {
        return Err(AppError::InvalidState(
            "Claude OAuth LLM returned valid JSON, but the response is not an object".to_string(),
        ));
    }
    serde_json::to_string(&value).map_err(|error| {
        AppError::InvalidState(format!("failed to normalize Claude OAuth LLM JSON: {error}"))
    })
}

fn now_ms() -> AppResult<u64> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| {
            AppError::InvalidState(format!("system clock is before Unix epoch: {error}"))
        })?
        .as_millis();
    u64::try_from(millis)
        .map_err(|_| AppError::InvalidState("system clock value is out of range".to_string()))
}

fn claude_request_error(status: StatusCode, body: &str) -> AppError {
    let detail = backend_error_message(body).unwrap_or_else(|| {
        status
            .canonical_reason()
            .unwrap_or("unknown error")
            .to_string()
    });
    let login_guidance = if status == StatusCode::UNAUTHORIZED {
        "; run `claude login` and retry"
    } else {
        ""
    };
    AppError::InvalidState(format!(
        "Claude OAuth LLM request failed with HTTP {status}: {detail}{login_guidance}"
    ))
}

fn backend_error_message(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    value
        .pointer("/error/message")
        .or_else(|| value.pointer("/response/error/message"))
        .or_else(|| value.get("message"))
        .or_else(|| value.get("detail"))
        .and_then(Value::as_str)
        .filter(|message| !message.trim().is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_claude_credential_shape_with_millisecond_epoch() {
        let future = now_ms().unwrap() + 86_400_000;
        let file = serde_json::json!({
            "claudeAiOauth": {
                "accessToken": "at-fixture",
                "refreshToken": "rt-fixture",
                "expiresAt": future as i64,
                "subscriptionType": "max"
            }
        });
        let credentials = parse_credentials(&file).expect("fixture should parse");
        assert_eq!(credentials.access_token, "at-fixture");
        assert_eq!(credentials.refresh_token, "rt-fixture");
        assert_eq!(credentials.expires_at_ms, Some(future));
    }

    #[test]
    fn flags_expired_and_missing_expiry_without_failing_login_state() {
        let past = i64::try_from(now_ms().unwrap() - 1_000).unwrap();
        let expired = serde_json::json!({
            "claudeAiOauth": {"accessToken": "a", "refreshToken": "r", "expiresAt": past}
        });
        let credentials = parse_credentials(&expired).expect("expired fixture should parse");
        assert!(credentials.expires_at_ms.unwrap() < now_ms().unwrap());

        let absent = serde_json::json!({
            "claudeAiOauth": {"accessToken": "a", "refreshToken": "r"}
        });
        let credentials = parse_credentials(&absent).expect("missing expiry should parse");
        assert_eq!(credentials.expires_at_ms, None);
    }

    #[test]
    fn rejects_credentials_missing_required_fields() {
        let missing_refresh = serde_json::json!({"claudeAiOauth": {"accessToken": "a"}});
        assert!(parse_credentials(&missing_refresh).is_err());
        let wrong_root = serde_json::json!({"somethingElse": {}});
        assert!(parse_credentials(&wrong_root).is_err());
    }

    #[test]
    fn extracts_text_blocks_from_messages_response() {
        let body = serde_json::json!({
            "content": [
                {"type": "text", "text": "{\"summary\": \"ok\", "},
                {"type": "tool_use", "id": "x"},
                {"type": "text", "text": "\"decisions\": []}"}
            ]
        });
        let text = response_text_from_messages(&body.to_string()).expect("text extraction");
        assert_eq!(text, "{\"summary\": \"ok\", \"decisions\": []}");
    }

    #[test]
    fn normalizes_fenced_and_plain_json_objects() {
        let fenced = "```json\n{\"summary\": \"출시\"}\n```";
        assert_eq!(
            normalize_json_output(fenced).expect("fenced json"),
            "{\"summary\":\"출시\"}"
        );
        let plain = "  {\"summary\": \"출시\"}  ";
        assert_eq!(
            normalize_json_output(plain).expect("plain json"),
            "{\"summary\":\"출시\"}"
        );
        assert!(normalize_json_output("[1,2,3]").is_err());
        assert!(normalize_json_output("not json").is_err());
    }
}
