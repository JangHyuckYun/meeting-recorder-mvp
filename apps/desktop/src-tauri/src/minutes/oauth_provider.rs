//! ChatGPT subscription-backed structured generation using Codex CLI OAuth credentials.
//!
//! The installed Codex CLI uses `~/.codex/auth.json`, refreshes at
//! `https://auth.openai.com/oauth/token`, and sends Responses requests to
//! `https://chatgpt.com/backend-api/codex/responses`. This module mirrors that contract without
//! requiring a separate API key.

use crate::error::{AppError, AppResult};
use chrono::{SecondsFormat, Utc};
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DEFAULT_CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api";
const DEFAULT_CODEX_MODEL: &str = "gpt-5.4-mini";
const OPENAI_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const OPENAI_CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const TOKEN_REFRESH_MARGIN_MS: u64 = 60_000;

#[derive(Debug)]
struct CodexCredentials {
    access_token: String,
    refresh_token: String,
    account_id: String,
}

#[derive(Debug, Deserialize)]
struct RefreshResponse {
    access_token: String,
    refresh_token: String,
    #[serde(rename = "expires_in")]
    _expires_in: u64,
}

#[derive(Debug, Deserialize)]
struct JwtClaims {
    exp: u64,
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
    let (mut auth_file, mut credentials) = load_credentials(&credentials_path)?;

    let expires_at_ms = jwt_expiry_ms(&credentials.access_token)?;
    if expires_at_ms <= now_ms()?.saturating_add(TOKEN_REFRESH_MARGIN_MS) {
        credentials = refresh_credentials(&client, &credentials, &credentials_path).await?;
        update_and_persist_credentials(&credentials_path, &mut auth_file, &credentials)?;
    }

    let base_url = std::env::var("MINUTES_OAUTH_BASE_URL")
        .unwrap_or_else(|_| DEFAULT_CODEX_BASE_URL.to_string());
    let model =
        std::env::var("MINUTES_OAUTH_MODEL").unwrap_or_else(|_| DEFAULT_CODEX_MODEL.to_string());
    let endpoint = format!(
        "{}/codex/responses",
        base_url.trim_end_matches('/').trim_end_matches("/codex")
    );
    let schema_json = serde_json::to_string(schema).map_err(|error| {
        AppError::InvalidState(format!(
            "failed to serialize OAuth LLM JSON schema: {error}"
        ))
    })?;
    let instructions = format!(
        "{system_prompt}\n\nReturn exactly one valid JSON object and no other text. Do not use Markdown fences. The object must conform to this JSON Schema:\n{schema_json}"
    );

    let response = client
        .post(endpoint)
        .bearer_auth(&credentials.access_token)
        .header("chatgpt-account-id", &credentials.account_id)
        .header("originator", "codex_cli_rs")
        .header("User-Agent", "codex_cli_rs")
        .header("OpenAI-Beta", "responses=experimental")
        .header("accept", "text/event-stream")
        .json(&json!({
            "model": model,
            "store": false,
            "stream": true,
            "instructions": instructions,
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": user_prompt}]
            }],
            "text": {"verbosity": "low"},
            "include": ["reasoning.encrypted_content"],
            "tool_choice": "auto",
            "parallel_tool_calls": true
        }))
        .send()
        .await?;
    let status = response.status();
    let response_body = response.text().await?;
    if !status.is_success() {
        return Err(oauth_request_error(status, &response_body));
    }

    let text = response_text_from_sse(&response_body)?;
    normalize_json_output(&text)
}

fn credentials_path() -> AppResult<PathBuf> {
    if let Ok(path) = std::env::var("MINUTES_OAUTH_CREDENTIALS_PATH") {
        if !path.trim().is_empty() {
            return Ok(PathBuf::from(path));
        }
    }

    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or_else(|| {
            AppError::InvalidState(
                "cannot locate Codex OAuth credentials because HOME is not set".to_string(),
            )
        })?;
    Ok(PathBuf::from(home).join(".codex/auth.json"))
}

fn load_credentials(path: &Path) -> AppResult<(Value, CodexCredentials)> {
    let contents = fs::read_to_string(path).map_err(|error| {
        AppError::InvalidState(format!(
            "cannot read Codex OAuth credentials at {}: {error}; run `codex login`",
            path.display()
        ))
    })?;
    let auth_file: Value = serde_json::from_str(&contents).map_err(|error| {
        AppError::InvalidState(format!(
            "Codex OAuth credentials at {} are invalid JSON: {error}; run `codex login`",
            path.display()
        ))
    })?;
    let tokens = auth_file
        .get("tokens")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_credentials_shape(path, "missing object `tokens`"))?;

    let read_token = |key: &str| {
        tokens
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned)
            .ok_or_else(|| invalid_credentials_shape(path, &format!("missing `tokens.{key}`")))
    };
    let credentials = CodexCredentials {
        access_token: read_token("access_token")?,
        refresh_token: read_token("refresh_token")?,
        account_id: read_token("account_id")?,
    };
    Ok((auth_file, credentials))
}

fn invalid_credentials_shape(path: &Path, detail: &str) -> AppError {
    AppError::InvalidState(format!(
        "Codex OAuth credentials at {} have an unsupported shape ({detail}); run `codex login`",
        path.display()
    ))
}

async fn refresh_credentials(
    client: &reqwest::Client,
    current: &CodexCredentials,
    path: &Path,
) -> AppResult<CodexCredentials> {
    let response = client
        .post(OPENAI_TOKEN_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", current.refresh_token.as_str()),
            ("client_id", OPENAI_CODEX_CLIENT_ID),
        ])
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
            "Codex OAuth token refresh failed with HTTP {status}: {detail}; run `codex login` and retry"
        )));
    }
    let refreshed: RefreshResponse = serde_json::from_str(&body).map_err(|error| {
        AppError::InvalidState(format!(
            "Codex OAuth token refresh returned invalid JSON: {error}; run `codex login` and retry"
        ))
    })?;
    if refreshed.access_token.trim().is_empty() || refreshed.refresh_token.trim().is_empty() {
        return Err(invalid_credentials_shape(
            path,
            "token refresh returned empty tokens",
        ));
    }

    Ok(CodexCredentials {
        access_token: refreshed.access_token,
        refresh_token: refreshed.refresh_token,
        account_id: current.account_id.clone(),
    })
}

fn update_and_persist_credentials(
    path: &Path,
    auth_file: &mut Value,
    credentials: &CodexCredentials,
) -> AppResult<()> {
    let root = auth_file
        .as_object_mut()
        .ok_or_else(|| invalid_credentials_shape(path, "root is not an object"))?;
    let tokens = root
        .get_mut("tokens")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid_credentials_shape(path, "missing object `tokens`"))?;
    tokens.insert(
        "access_token".to_string(),
        Value::String(credentials.access_token.clone()),
    );
    tokens.insert(
        "refresh_token".to_string(),
        Value::String(credentials.refresh_token.clone()),
    );
    root.insert(
        "last_refresh".to_string(),
        Value::String(Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)),
    );

    let parent = path.parent().ok_or_else(|| {
        AppError::InvalidState(format!(
            "Codex OAuth credential path {} has no parent directory",
            path.display()
        ))
    })?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("auth.json");
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
        serde_json::to_writer_pretty(&mut temporary, auth_file).map_err(|error| {
            AppError::InvalidState(format!(
                "failed to serialize refreshed Codex OAuth credentials: {error}"
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
            "refreshed the Codex OAuth token but could not safely update {}: {error}",
            path.display()
        ))
    })
}

fn jwt_expiry_ms(token: &str) -> AppResult<u64> {
    let payload = token.split('.').nth(1).ok_or_else(|| {
        AppError::InvalidState(
            "Codex OAuth access token is not a JWT; run `codex login` and retry".to_string(),
        )
    })?;
    let decoded = decode_base64_url(payload).map_err(|detail| {
        AppError::InvalidState(format!(
            "cannot read Codex OAuth token expiry ({detail}); run `codex login` and retry"
        ))
    })?;
    let claims: JwtClaims = serde_json::from_slice(&decoded).map_err(|error| {
        AppError::InvalidState(format!(
            "cannot read Codex OAuth token expiry: {error}; run `codex login` and retry"
        ))
    })?;
    claims.exp.checked_mul(1_000).ok_or_else(|| {
        AppError::InvalidState("Codex OAuth token expiry is out of range".to_string())
    })
}

fn decode_base64_url(input: &str) -> Result<Vec<u8>, &'static str> {
    let mut output = Vec::with_capacity(input.len() * 3 / 4);
    let mut buffer = 0_u32;
    let mut buffered_bits = 0_u8;
    for byte in input.bytes() {
        if byte == b'=' {
            break;
        }
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return Err("invalid base64url character"),
        };
        buffer = (buffer << 6) | u32::from(value);
        buffered_bits += 6;
        if buffered_bits >= 8 {
            buffered_bits -= 8;
            output.push((buffer >> buffered_bits) as u8);
            buffer &= if buffered_bits == 0 {
                0
            } else {
                (1_u32 << buffered_bits) - 1
            };
        }
    }
    if buffered_bits >= 6 {
        return Err("invalid base64url length");
    }
    Ok(output)
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

fn oauth_request_error(status: StatusCode, body: &str) -> AppError {
    let detail = backend_error_message(body).unwrap_or_else(|| {
        status
            .canonical_reason()
            .unwrap_or("unknown error")
            .to_string()
    });
    let login_guidance = if status == StatusCode::UNAUTHORIZED {
        "; run `codex login` and retry"
    } else {
        ""
    };
    AppError::InvalidState(format!(
        "Codex OAuth LLM request failed with HTTP {status}: {detail}{login_guidance}"
    ))
}

fn backend_error_message(body: &str) -> Option<String> {
    let message_from_value = |value: &Value| {
        value
            .pointer("/error/message")
            .or_else(|| value.pointer("/response/error/message"))
            .or_else(|| value.get("message"))
            .or_else(|| value.get("detail"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
    };
    if let Ok(value) = serde_json::from_str::<Value>(body) {
        if let Some(message) = message_from_value(&value) {
            return Some(message);
        }
    }
    for data in sse_data_values(body) {
        if let Ok(value) = serde_json::from_str::<Value>(&data) {
            if let Some(message) = message_from_value(&value) {
                return Some(message);
            }
        }
    }
    None
}

fn response_text_from_sse(body: &str) -> AppResult<String> {
    let mut output = String::new();
    let mut completed_text = None;
    for data in sse_data_values(body) {
        if data == "[DONE]" {
            continue;
        }
        let event: Value = serde_json::from_str(&data).map_err(|error| {
            AppError::InvalidState(format!(
                "Codex OAuth LLM returned malformed SSE JSON: {error}"
            ))
        })?;
        match event.get("type").and_then(Value::as_str) {
            Some("response.output_text.delta") => {
                if let Some(delta) = event.get("delta").and_then(Value::as_str) {
                    output.push_str(delta);
                }
            }
            Some("response.output_text.done") => {
                completed_text = event
                    .get("text")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
            }
            Some("error" | "response.failed") => {
                let message = backend_error_message(&data)
                    .unwrap_or_else(|| "the backend reported an unspecified error".to_string());
                return Err(AppError::InvalidState(format!(
                    "Codex OAuth LLM stream failed: {message}"
                )));
            }
            _ => {}
        }
    }
    if output.trim().is_empty() {
        output = completed_text.unwrap_or_default();
    }
    if output.trim().is_empty() {
        return Err(AppError::InvalidState(
            "Codex OAuth LLM returned no response text".to_string(),
        ));
    }
    Ok(output)
}

fn sse_data_values(body: &str) -> Vec<String> {
    body.replace("\r\n", "\n")
        .split("\n\n")
        .filter_map(|block| {
            let data = block
                .lines()
                .filter_map(|line| line.strip_prefix("data:"))
                .map(str::trim_start)
                .collect::<Vec<_>>()
                .join("\n");
            (!data.is_empty()).then_some(data)
        })
        .collect()
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
            "Codex OAuth LLM returned malformed JSON instead of the requested schema: {error}"
        ))
    })?;
    if !value.is_object() {
        return Err(AppError::InvalidState(
            "Codex OAuth LLM returned valid JSON, but the response is not an object".to_string(),
        ));
    }
    serde_json::to_string(&value).map_err(|error| {
        AppError::InvalidState(format!("failed to normalize Codex OAuth LLM JSON: {error}"))
    })
}
