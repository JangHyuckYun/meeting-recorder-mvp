use crate::error::{AppError, AppResult};
use crate::minutes::oauth_status::OAuthStatus;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{mpsc, Mutex};

const OPENAI_CLIENT: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const CLAUDE_CLIENT: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";

#[derive(Debug, serde::Serialize)]
pub struct AuthorizeResponse { pub authorize_url: String }

pub struct PendingLogin {
    provider: String,
    verifier: String,
    state: String,
    callback: mpsc::Receiver<String>,
}

pub async fn start(slot: &Mutex<Option<PendingLogin>>, provider: &str) -> AppResult<AuthorizeResponse> {
    let (port, redirect, state) = match provider {
        "openai" => (1455, "http://localhost:1455/auth/callback", random_value()),
        "anthropic" => (53692, "http://localhost:53692/callback", random_value()),
        _ => return Err(AppError::InvalidState("provider must be openai or anthropic".into())),
    };
    let verifier = random_value();
    let challenge = pkce_challenge(&verifier);
    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|e| AppError::InvalidState(format!("OAuth callback port {port} is busy; paste the code or redirect URL instead ({e})")))?;
    let (sender, callback) = mpsc::channel();
    *slot.lock().unwrap() = Some(PendingLogin { provider: provider.into(), verifier: verifier.clone(), state: state.clone(), callback });
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0; 8192]; let _ = stream.read(&mut request);
            let line = String::from_utf8_lossy(&request);
            if let Some(path) = line.lines().next().and_then(|line| line.split_whitespace().nth(1)) {
                let _ = sender.send(path.to_string());
            }
            let body = "<meta charset='utf-8'><h2>인증 완료, 창을 닫으세요</h2>";
            let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
            let _ = stream.write_all(response.as_bytes());
        }
    });
    let url = if provider == "openai" {
        format!("https://auth.openai.com/oauth/authorize?response_type=code&client_id={OPENAI_CLIENT}&redirect_uri={redirect}&scope=openid%20profile%20email%20offline_access&code_challenge={challenge}&code_challenge_method=S256&state={state}")
    } else {
        format!("https://claude.ai/oauth/authorize?code=true&client_id={CLAUDE_CLIENT}&response_type=code&redirect_uri={redirect}&scope=org:create_api_key%20user:profile%20user:inference&code_challenge={challenge}&code_challenge_method=S256&state={verifier}")
    };
    let _ = std::process::Command::new("open").arg(&url).spawn();
    Ok(AuthorizeResponse { authorize_url: url })
}

pub async fn complete(slot: &Mutex<Option<PendingLogin>>, provider: &str, pasted: Option<String>) -> AppResult<OAuthStatus> {
    let (pending, raw) = {
        let mut guard = slot.lock().unwrap();
        let p = guard.as_mut().ok_or_else(|| AppError::InvalidState("no OAuth login in progress".into()))?;
        if p.provider != provider { return Err(AppError::InvalidState("OAuth provider does not match pending login".into())); }
        let raw = pasted.or_else(|| p.callback.try_recv().ok()).ok_or_else(|| AppError::InvalidState("OAuth callback not received; paste the code or redirect URL".into()))?;
        (guard.take().expect("pending OAuth login remains present"), raw)
    };
    let (code, returned_state) = parse_code_state(&raw).ok_or_else(|| AppError::InvalidState("could not find OAuth code".into()))?;
    let expected_state = if provider == "anthropic" { &pending.verifier } else { &pending.state };
    if returned_state.as_deref().is_some_and(|state| state != expected_state) {
        return Err(AppError::InvalidState("OAuth state does not match".into()));
    }
    let client = reqwest::Client::new();
    if provider == "openai" { exchange_openai(&client, &code, &pending.verifier).await?; }
    else { exchange_claude(&client, &code, returned_state.as_deref().unwrap_or(&pending.verifier), &pending.verifier).await?; }
    crate::minutes::oauth_status::inspect(if provider == "openai" { "codex_oauth" } else { "claude_oauth" })
}

async fn exchange_openai(client: &reqwest::Client, code: &str, verifier: &str) -> AppResult<()> {
    let value: Value = client.post("https://auth.openai.com/oauth/token").form(&[("grant_type", "authorization_code"), ("code", code), ("code_verifier", verifier), ("redirect_uri", "http://localhost:1455/auth/callback"), ("client_id", OPENAI_CLIENT)]).send().await?.error_for_status()?.json().await?;
    let access = value["access_token"].as_str().filter(|v| !v.is_empty()).ok_or_else(|| AppError::InvalidState("OpenAI OAuth response has no access token".into()))?;
    let refresh = value["refresh_token"].as_str().unwrap_or("");
    let account_id = jwt_account_id(access).unwrap_or_default();
    atomic_write(&codex_path(), &json!({"tokens":{"access_token":access,"refresh_token":refresh,"account_id":account_id}}))
}

async fn exchange_claude(client: &reqwest::Client, code: &str, state: &str, verifier: &str) -> AppResult<()> {
    let value: Value = client.post("https://platform.claude.com/v1/oauth/token").json(&json!({"grant_type":"authorization_code","client_id":CLAUDE_CLIENT,"code":code,"state":state,"redirect_uri":"http://localhost:53692/callback","code_verifier":verifier})).send().await?.error_for_status()?.json().await?;
    let access = value["access_token"].as_str().unwrap_or(""); let refresh = value["refresh_token"].as_str().unwrap_or("");
    if access.is_empty() || refresh.is_empty() { return Err(AppError::InvalidState("Claude OAuth response has incomplete credentials".into())); }
    let expires = value["expires_in"].as_u64().map(|s| chrono::Utc::now().timestamp_millis() + (s as i64 * 1000));
    atomic_write(&claude_path(), &json!({"claudeAiOauth":{"accessToken":access,"refreshToken":refresh,"expiresAt":expires,"scopes":["org:create_api_key","user:profile","user:inference"]}}))
}

pub fn pkce_challenge(verifier: &str) -> String { base64_url(&Sha256::digest(verifier.as_bytes())) }

pub fn parse_code_state(input: &str) -> Option<(String, Option<String>)> {
    let input = input.trim();
    if input.contains("#") && !input.contains("?") { let mut p = input.splitn(2, '#'); return Some((p.next()?.to_string(), Some(p.next()?.to_string()))); }
    let query = input.split_once('?').map(|(_, q)| q).unwrap_or(input).split('&');
    let mut code = None; let mut state = None;
    for item in query { let (k, v) = item.split_once('=')?; if k == "code" { code = Some(percent_decode(v)) } else if k == "state" { state = Some(percent_decode(v)) } }
    code.map(|code| (code, state))
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(high), Some(low)) = (hex_digit(bytes[i + 1]), hex_digit(bytes[i + 2])) {
                out.push(high * 16 + low);
                i += 3;
                continue;
            }
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
fn hex_digit(value: u8) -> Option<u8> { match value { b'0'..=b'9' => Some(value - b'0'), b'a'..=b'f' => Some(value - b'a' + 10), b'A'..=b'F' => Some(value - b'A' + 10), _ => None } }

fn random_value() -> String { format!("{:x}", Sha256::digest(format!("{}:{}", std::process::id(), chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()).as_bytes())) }
fn base64_url(bytes: &[u8]) -> String { const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"; let mut out = String::new(); let mut n = 0u32; let mut bits = 0; for &b in bytes { n = (n << 8) | b as u32; bits += 8; while bits >= 6 { bits -= 6; out.push(A[((n >> bits) & 63) as usize] as char); } } if bits > 0 { out.push(A[((n << (6 - bits)) & 63) as usize] as char); } out }
fn jwt_account_id(token: &str) -> Option<String> { let payload = token.split('.').nth(1)?; let mut bytes = Vec::new(); let mut n = 0u32; let mut bits = 0; for b in payload.bytes() { let v = match b { b'A'..=b'Z' => b-65, b'a'..=b'z' => b-71, b'0'..=b'9' => b+4, b'-' => 62, b'_' => 63, _ => continue }; n=(n<<6)|v as u32; bits+=6; if bits>=8 { bits-=8; bytes.push((n>>bits) as u8); n &= (1<<bits)-1; } } let v: Value=serde_json::from_slice(&bytes).ok()?; v["https://api.openai.com/auth"]["chatgpt_account_id"].as_str().or_else(||v["account_id"].as_str()).map(str::to_owned) }
fn codex_path() -> PathBuf { std::env::var("MINUTES_CODEX_CREDENTIALS_PATH").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".codex/auth.json")) }
fn claude_path() -> PathBuf { std::env::var("MINUTES_CLAUDE_CREDENTIALS_PATH").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".claude/.credentials.json")) }
fn atomic_write(path: &PathBuf, value: &Value) -> AppResult<()> { if let Some(parent)=path.parent(){fs::create_dir_all(parent)?;} let tmp=path.with_extension(format!("tmp-{}",std::process::id())); let mut f=OpenOptions::new().create_new(true).write(true).open(&tmp)?; #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; f.set_permissions(fs::Permissions::from_mode(0o600))?; } serde_json::to_writer_pretty(&mut f,value).map_err(|e|AppError::InvalidState(e.to_string()))?; f.write_all(b"\n")?; f.sync_all()?; drop(f); fs::rename(tmp,path)?; Ok(()) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn pkce_vector(){ assert_eq!(pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"); }
    #[test] fn parses_inputs(){ assert_eq!(parse_code_state("code#state"),Some(("code".into(),Some("state".into())))); assert_eq!(parse_code_state("http://localhost/callback?code=x%2Dy&state=y"),Some(("x-y".into(),Some("y".into())))); }
    #[test] fn writes_provider_credential_shapes() {
        let dir = tempfile::tempdir().unwrap();
        let codex = dir.path().join("auth.json");
        atomic_write(&codex, &json!({"tokens":{"access_token":"a","refresh_token":"r","account_id":"id"}})).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&fs::read_to_string(codex).unwrap()).unwrap()["tokens"]["account_id"], "id");
        let claude = dir.path().join("claude.json");
        atomic_write(&claude, &json!({"claudeAiOauth":{"accessToken":"a","refreshToken":"r","expiresAt":1,"scopes":[]}})).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&fs::read_to_string(claude).unwrap()).unwrap()["claudeAiOauth"]["accessToken"], "a");
    }
}
