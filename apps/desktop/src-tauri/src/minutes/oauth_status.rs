//! Credential-status inspection for the settings UI. Reads the same credential stores
//! as the OAuth providers but never performs network refreshes — the UI only reflects
//! what is on disk.

use crate::error::{AppError, AppResult};
use chrono::{SecondsFormat, TimeZone, Utc};
use serde_json::Value;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// What the frontend SettingsModal renders per provider.
#[derive(Debug, Clone, serde::Serialize)]
pub struct OAuthStatus {
    pub provider: String,
    pub logged_in: bool,
    pub account_id: Option<String>,
    pub expires_at: Option<String>,
    pub access_expired: bool,
    pub credentials_path: String,
}

/// Tauri-facing entry: resolves which credential store to inspect.
pub fn inspect(provider: &str) -> AppResult<OAuthStatus> {
    match provider {
        "codex_oauth" => codex_status(),
        "claude_oauth" => claude_status(),
        other => Err(AppError::InvalidState(format!(
            "unknown oauth provider `{other}`; expected `codex_oauth` or `claude_oauth`"
        ))),
    }
}

fn codex_credentials_path() -> PathBuf {
    if let Ok(path) = std::env::var("MINUTES_OAUTH_CREDENTIALS_PATH") {
        if !path.trim().is_empty() {
            return PathBuf::from(path);
        }
    }
    if let Ok(path) = std::env::var("MINUTES_CODEX_CREDENTIALS_PATH") {
        if !path.trim().is_empty() {
            return PathBuf::from(path);
        }
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join(".codex/auth.json")
}

fn claude_credentials_path() -> PathBuf {
    if let Ok(path) = std::env::var("MINUTES_CLAUDE_CREDENTIALS_PATH") {
        if !path.trim().is_empty() {
            return PathBuf::from(path);
        }
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join(".claude/.credentials.json")
}

fn codex_status() -> AppResult<OAuthStatus> {
    let path = codex_credentials_path();
    let path_str = path.display().to_string();
    let contents = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(_) => {
            return Ok(OAuthStatus {
                provider: "codex_oauth".to_string(),
                logged_in: false,
                account_id: None,
                expires_at: None,
                access_expired: false,
                credentials_path: path_str,
            })
        }
    };
    let parsed: Value = match serde_json::from_str(&contents) {
        Ok(value) => value,
        Err(_) => {
            return Ok(OAuthStatus {
                provider: "codex_oauth".to_string(),
                logged_in: false,
                account_id: None,
                expires_at: None,
                access_expired: false,
                credentials_path: path_str,
            })
        }
    };
    let tokens = parsed.get("tokens").and_then(Value::as_object);
    let Some(tokens) = tokens else {
        return Ok(OAuthStatus {
            provider: "codex_oauth".to_string(),
            logged_in: false,
            account_id: None,
            expires_at: None,
            access_expired: false,
            credentials_path: path_str,
        });
    };
    let access_token = tokens
        .get("access_token")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let refresh_token = tokens
        .get("refresh_token")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let account_id = tokens
        .get("account_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.to_string());

    let logged_in = !refresh_token.is_empty() && !access_token.is_empty();
    if !logged_in {
        return Ok(OAuthStatus {
            provider: "codex_oauth".to_string(),
            logged_in: false,
            account_id,
            expires_at: None,
            access_expired: false,
            credentials_path: path_str,
        });
    }

    let (expires_at, access_expired) = match jwt_expiry_ms(&access_token) {
        Ok(exp_ms) => {
            let expiry = Utc
                .timestamp_millis_opt(exp_ms as i64)
                .single()
                .map(|dt| dt.to_rfc3339_opts(SecondsFormat::Millis, true));
            let expired = now_ms().map(|now| now >= exp_ms).unwrap_or(false);
            (expiry, expired)
        }
        Err(_) => (None, false),
    };

    Ok(OAuthStatus {
        provider: "codex_oauth".to_string(),
        logged_in: true,
        account_id,
        expires_at,
        access_expired,
        credentials_path: path_str,
    })
}

fn claude_status() -> AppResult<OAuthStatus> {
    let path = claude_credentials_path();
    let path_str = path.display().to_string();
    let contents = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(_) => {
            return Ok(OAuthStatus {
                provider: "claude_oauth".to_string(),
                logged_in: false,
                account_id: None,
                expires_at: None,
                access_expired: false,
                credentials_path: path_str,
            })
        }
    };
    let parsed: Value = match serde_json::from_str(&contents) {
        Ok(value) => value,
        Err(_) => {
            return Ok(OAuthStatus {
                provider: "claude_oauth".to_string(),
                logged_in: false,
                account_id: None,
                expires_at: None,
                access_expired: false,
                credentials_path: path_str,
            })
        }
    };
    let oauth = parsed.get("claudeAiOauth");
    let Some(oauth) = oauth else {
        return Ok(OAuthStatus {
            provider: "claude_oauth".to_string(),
            logged_in: false,
            account_id: None,
            expires_at: None,
            access_expired: false,
            credentials_path: path_str,
        });
    };
    let refresh_token = oauth
        .get("refreshToken")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let logged_in = !refresh_token.is_empty();
    if !logged_in {
        return Ok(OAuthStatus {
            provider: "claude_oauth".to_string(),
            logged_in: false,
            account_id: None,
            expires_at: None,
            access_expired: false,
            credentials_path: path_str,
        });
    }

    let expires_at_ms = oauth.get("expiresAt").and_then(Value::as_i64).and_then(|value| {
        if value < 0 {
            None
        } else {
            Some(value as u64)
        }
    });
    let (expires_at, access_expired) = match expires_at_ms {
        Some(ms) => {
            let expiry = Utc
                .timestamp_millis_opt(ms as i64)
                .single()
                .map(|dt| dt.to_rfc3339_opts(SecondsFormat::Millis, true));
            let expired = now_ms().map(|now| now >= ms).unwrap_or(false);
            (expiry, expired)
        }
        None => (None, false),
    };

    Ok(OAuthStatus {
        provider: "claude_oauth".to_string(),
        logged_in: true,
        account_id: None,
        expires_at,
        access_expired,
        credentials_path: path_str,
    })
}

fn jwt_expiry_ms(token: &str) -> AppResult<u64> {
    let payload = token.split('.').nth(1).ok_or_else(|| {
        AppError::InvalidState("Codex OAuth access token is not a JWT".to_string())
    })?;
    let decoded = decode_base64_url(payload).map_err(|detail| {
        AppError::InvalidState(format!("cannot read Codex OAuth token expiry ({detail})"))
    })?;
    let claims: serde_json::Value = serde_json::from_slice(&decoded).map_err(|error| {
        AppError::InvalidState(format!("cannot read Codex OAuth token expiry: {error}"))
    })?;
    let exp = claims.get("exp").and_then(Value::as_u64).ok_or_else(|| {
        AppError::InvalidState("Codex OAuth token has no exp claim".to_string())
    })?;
    exp.checked_mul(1_000).ok_or_else(|| {
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
    u64::try_from(millis).map_err(|_| {
        AppError::InvalidState("system clock value is out of range".to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;

    static ENV_GUARD: Mutex<()> = Mutex::new(());

    fn write_temp_file(contents: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("credentials.json");
        fs::write(&path, contents).expect("write fixture");
        (dir, path)
    }

    fn codex_fixture(exp_offset_seconds: i64, refresh_present: bool) -> Value {
        let exp = (now_ms().unwrap() as i64 / 1000) + exp_offset_seconds;
        let header = base64_url_encode(r#"{"alg":"RS256","typ":"JWT"}"#);
        let payload = base64_url_encode(&format!(r#"{{"exp":{exp}}}"#));
        let token = format!("{header}.{payload}.sig");
        let mut tokens = serde_json::json!({
            "access_token": token,
            "account_id": "codex-test-account"
        });
        if refresh_present {
            tokens["refresh_token"] = serde_json::json!("rt-fixture");
        }
        serde_json::json!({"tokens": tokens})
    }

    fn base64_url_encode(input: &str) -> String {
        let bytes = input.as_bytes();
        let mut output = String::new();
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut buffer = 0_u32;
        let mut bits = 0_u8;
        for &byte in bytes {
            buffer = (buffer << 8) | u32::from(byte);
            bits += 8;
            while bits >= 6 {
                bits -= 6;
                output.push(char::from(alphabet[((buffer >> bits) & 0x3F) as usize]));
                buffer &= (1_u32 << bits).wrapping_sub(1);
            }
        }
        if bits > 0 {
            output.push(char::from(alphabet[((buffer << (6 - bits)) & 0x3F) as usize]));
        }
        output
    }

    #[test]
    fn codex_future_expiry_is_not_expired() {
        let _guard = ENV_GUARD.lock().unwrap();
        let fixture = codex_fixture(3600, true);
        let (_dir, path) = write_temp_file(&fixture.to_string());
        unsafe { std::env::set_var("MINUTES_OAUTH_CREDENTIALS_PATH", path.display().to_string()) };
        let status = codex_status().expect("status should succeed");
        assert!(status.logged_in);
        assert!(!status.access_expired);
        assert!(status.expires_at.is_some());
        assert_eq!(status.account_id.as_deref(), Some("codex-test-account"));
        unsafe { std::env::remove_var("MINUTES_OAUTH_CREDENTIALS_PATH") };
    }

    #[test]
    fn codex_expired_token_is_flagged() {
        let _guard = ENV_GUARD.lock().unwrap();
        let fixture = codex_fixture(-3600, true);
        let (_dir, path) = write_temp_file(&fixture.to_string());
        unsafe { std::env::set_var("MINUTES_OAUTH_CREDENTIALS_PATH", path.display().to_string()) };
        let status = codex_status().expect("status should succeed");
        assert!(status.logged_in);
        assert!(status.access_expired);
        unsafe { std::env::remove_var("MINUTES_OAUTH_CREDENTIALS_PATH") };
    }

    #[test]
    fn codex_missing_refresh_is_not_logged_in() {
        let _guard = ENV_GUARD.lock().unwrap();
        let fixture = codex_fixture(3600, false);
        let (_dir, path) = write_temp_file(&fixture.to_string());
        unsafe { std::env::set_var("MINUTES_OAUTH_CREDENTIALS_PATH", path.display().to_string()) };
        let status = codex_status().expect("status should succeed");
        assert!(!status.logged_in);
        unsafe { std::env::remove_var("MINUTES_OAUTH_CREDENTIALS_PATH") };
    }

    #[test]
    fn claude_future_expiry_is_not_expired() {
        let _guard = ENV_GUARD.lock().unwrap();
        let future = now_ms().unwrap() + 86_400_000;
        let fixture = serde_json::json!({
            "claudeAiOauth": {
                "accessToken": "at-fixture",
                "refreshToken": "rt-fixture",
                "expiresAt": future as i64
            }
        });
        let (_dir, path) = write_temp_file(&fixture.to_string());
        unsafe { std::env::set_var("MINUTES_CLAUDE_CREDENTIALS_PATH", path.display().to_string()) };
        let status = claude_status().expect("status should succeed");
        assert!(status.logged_in);
        assert!(!status.access_expired);
        assert!(status.expires_at.is_some());
        unsafe { std::env::remove_var("MINUTES_CLAUDE_CREDENTIALS_PATH") };
    }

    #[test]
    fn claude_expired_is_flagged() {
        let _guard = ENV_GUARD.lock().unwrap();
        let past = (now_ms().unwrap() - 5_000) as i64;
        let fixture = serde_json::json!({
            "claudeAiOauth": {
                "accessToken": "at-fixture",
                "refreshToken": "rt-fixture",
                "expiresAt": past
            }
        });
        let (_dir, path) = write_temp_file(&fixture.to_string());
        unsafe { std::env::set_var("MINUTES_CLAUDE_CREDENTIALS_PATH", path.display().to_string()) };
        let status = claude_status().expect("status should succeed");
        assert!(status.logged_in);
        assert!(status.access_expired);
        unsafe { std::env::remove_var("MINUTES_CLAUDE_CREDENTIALS_PATH") };
    }

    #[test]
    fn claude_missing_refresh_is_not_logged_in() {
        let _guard = ENV_GUARD.lock().unwrap();
        let fixture = serde_json::json!({
            "claudeAiOauth": {
                "accessToken": "at-fixture",
                "expiresAt": (now_ms().unwrap() + 86_400_000) as i64
            }
        });
        let (_dir, path) = write_temp_file(&fixture.to_string());
        unsafe { std::env::set_var("MINUTES_CLAUDE_CREDENTIALS_PATH", path.display().to_string()) };
        let status = claude_status().expect("status should succeed");
        assert!(!status.logged_in);
        unsafe { std::env::remove_var("MINUTES_CLAUDE_CREDENTIALS_PATH") };
    }

    #[test]
    fn unknown_provider_is_rejected() {
        assert!(inspect("unknown").is_err());
    }
}
