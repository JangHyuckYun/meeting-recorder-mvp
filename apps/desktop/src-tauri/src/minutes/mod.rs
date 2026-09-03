//! LLM-backed generation and focused editing of grounded meeting minutes.

pub(crate) mod cache;
mod claude_provider;
mod oauth_provider;
pub(crate) mod oauth_status;
pub(crate) mod oauth_login;

use crate::error::{AppError, AppResult};
use crate::models::{
    AskAnswer, AskSource, CaptionEvent, CaptionStatus, LlmProvider, MinutesDraft, MinutesItem,
    ProviderType, TranscriptSegment,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::time::Duration;
use uuid::Uuid;

const DEFAULT_LITELLM_BASE_URL: &str = "http://192.168.1.189:4000";
const DEFAULT_LITELLM_MODEL: &str = "gpt-5.6-terra";

#[derive(Debug, Clone, Copy)]
pub struct ModelOptions<'a> {
    pub model: &'a str,
    pub reasoning_effort: Option<&'a str>,
    pub fast: bool,
}

pub(crate) fn apply_model_options(
    body: &mut Value,
    provider_type: ProviderType,
    options: Option<&ModelOptions<'_>>,
) -> bool {
    let Some(options) = options else { return false };
    match provider_type {
        ProviderType::Openai | ProviderType::OpenaiCompatible => {
            if !options.model.starts_with("gpt-5") && options.reasoning_effort.is_none() {
                body["temperature"] = json!(0.1);
            }
            if let Some(effort) = options.reasoning_effort {
                body["reasoning_effort"] = json!(effort);
            }
            if options.fast {
                body["service_tier"] = json!("priority");
            }
            false
        }
        ProviderType::Anthropic => {
            let no_temperature = options.model.starts_with("claude-fable-5")
                || options.model.starts_with("claude-opus-5")
                || options.model.starts_with("claude-sonnet-5")
                || options.model == "claude-opus-4-7"
                || options.model == "claude-opus-4-8";
            body["max_tokens"] = json!(if options.reasoning_effort.is_some() { 16000 } else { 4096 });
            if !no_temperature { body["temperature"] = json!(0.1); }
            if let Some(effort) = options.reasoning_effort {
                body["thinking"] = json!({"type": "adaptive"});
                body["output_config"] = json!({"effort": effort});
            }
            let fast_supported = options.model.starts_with("claude-opus-5")
                || options.model == "claude-opus-4-8";
            if options.fast && fast_supported { body["speed"] = json!("fast"); }
            options.fast && fast_supported
        }
    }
}

#[derive(Debug, Deserialize)]
struct GeneratedMinutes {
    summary: String,
    decisions: Vec<GeneratedItem>,
    action_items: Vec<GeneratedItem>,
}

#[derive(Debug, Deserialize)]
struct GeneratedItem {
    text: String,
    #[serde(default)]
    evidence_segment_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct EditedText {
    text: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletion {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: String,
}

/// Generates Korean minutes from final transcript segments. Items whose evidence is missing,
/// malformed, or not part of the supplied transcript are dropped before the draft is returned.
pub async fn generate_minutes(
    provider: LlmProvider,
    recording_id: Uuid,
    segments: &[TranscriptSegment],
    template: Option<&str>,
    options: Option<&ModelOptions<'_>>,
) -> AppResult<MinutesDraft> {
    let final_segments: Vec<&TranscriptSegment> = segments
        .iter()
        .filter(|segment| segment.is_final && !segment.text.trim().is_empty())
        .collect();
    if final_segments.is_empty() {
        return Err(AppError::InvalidState(
            "cannot generate minutes without final transcript segments".to_string(),
        ));
    }

    let transcript = final_segments
        .iter()
        .map(|segment| {
            json!({
                "id": segment.id,
                "start_ms": segment.start_ms,
                "end_ms": segment.end_ms,
                "speaker_label": segment.speaker_label,
                "text": segment.text,
            })
        })
        .collect::<Vec<_>>();
    let transcript_json = serde_json::to_string(&transcript).map_err(|error| {
        AppError::InvalidState(format!("failed to serialize transcript for LLM: {error}"))
    })?;

    let system_prompt = minutes_system_prompt(template);
    let user_prompt = format!(
        "다음 전사 세그먼트로 한국어 회의록을 작성하세요. 각 결정과 할 일에 직접 관련된 세그먼트 id를 인용하세요.\n\n{transcript_json}"
    );

    let content =
        request_structured_json(provider, &system_prompt, &user_prompt, minutes_schema(), options).await?;
    let valid_segment_ids = final_segments
        .iter()
        .map(|segment| segment.id)
        .collect::<Vec<_>>();
    parse_minutes_response(recording_id, &content, &valid_segment_ids)
}

/// Parses the JSON object returned by the LLM into the domain draft while enforcing evidence
/// references against the transcript supplied by the caller.
pub fn parse_minutes_response(
    recording_id: Uuid,
    response: &str,
    valid_segment_ids: &[Uuid],
) -> AppResult<MinutesDraft> {
    let generated: GeneratedMinutes = serde_json::from_str(response).map_err(|error| {
        AppError::InvalidState(format!("LLM returned invalid minutes JSON: {error}"))
    })?;
    let summary = generated.summary.trim().to_string();
    if summary.is_empty() {
        return Err(AppError::InvalidState(
            "LLM returned an empty minutes summary".to_string(),
        ));
    }

    let valid_ids = valid_segment_ids.iter().copied().collect::<HashSet<_>>();
    Ok(MinutesDraft {
        recording_id,
        summary,
        decisions: grounded_items(generated.decisions, &valid_ids),
        action_items: grounded_items(generated.action_items, &valid_ids),
        updated_at: Utc::now(),
    })
}

/// Returns only replacement text for one item. The caller remains responsible for applying that
/// text to the existing item so its id, evidence references, and neighboring items stay intact.
pub async fn edit_minutes_item_text(
    provider: LlmProvider,
    item: &MinutesItem,
    instruction: &str,
    evidence_segments: &[TranscriptSegment],
    options: Option<&ModelOptions<'_>>,
) -> AppResult<String> {
    let instruction = instruction.trim();
    if instruction.is_empty() {
        return Err(AppError::InvalidState(
            "minutes edit instruction cannot be empty".to_string(),
        ));
    }
    if evidence_segments.is_empty() {
        return Err(AppError::InvalidState(format!(
            "minutes item {} has no available evidence segments",
            item.id
        )));
    }

    let evidence = evidence_segments
        .iter()
        .map(|segment| {
            json!({
                "id": segment.id,
                "speaker_label": segment.speaker_label,
                "text": segment.text,
            })
        })
        .collect::<Vec<_>>();
    let user_prompt = serde_json::to_string(&json!({
        "current_text": item.text,
        "instruction": instruction,
        "evidence": evidence,
    }))
    .map_err(|error| {
        AppError::InvalidState(format!("failed to serialize minutes edit request: {error}"))
    })?;

    let system_prompt = concat!(
        "당신은 한국어 회의록의 항목 하나만 수정합니다. 사용자 지시를 따르되 evidence에 없는 사실을 추가하지 마세요. ",
        "다른 항목, id, evidence_segment_ids는 수정할 수 없습니다. 수정된 한국어 text 하나만 JSON 스키마에 맞춰 반환하세요."
    );
    let content =
        request_structured_json(provider, system_prompt, &user_prompt, edit_schema(), options).await?;
    let edited: EditedText = serde_json::from_str(&content).map_err(|error| {
        AppError::InvalidState(format!("LLM returned invalid minutes edit JSON: {error}"))
    })?;
    let text = edited.text.trim().to_string();
    if text.is_empty() {
        return Err(AppError::InvalidState(
            "LLM returned empty text for minutes edit".to_string(),
        ));
    }
    Ok(text)
}

fn grounded_items(items: Vec<GeneratedItem>, valid_ids: &HashSet<Uuid>) -> Vec<MinutesItem> {
    items
        .into_iter()
        .filter_map(|item| {
            let text = item.text.trim().to_string();
            let mut seen = HashSet::new();
            let evidence_segment_ids = item
                .evidence_segment_ids
                .into_iter()
                .filter_map(|id| Uuid::parse_str(&id).ok())
                .filter(|id| valid_ids.contains(id) && seen.insert(*id))
                .collect::<Vec<_>>();

            if text.is_empty() || evidence_segment_ids.is_empty() {
                None
            } else {
                Some(MinutesItem {
                    id: Uuid::new_v4(),
                    text,
                    evidence_segment_ids,
                })
            }
        })
        .collect()
}

/// Routes one structured-generation request to the LLM backend selected in app settings.
/// The provider is injected explicitly by the command layer — there is deliberately no
/// environment-variable fallback, so the settings UI is the single source of truth.
async fn request_structured_json(
    provider: LlmProvider,
    system_prompt: &str,
    user_prompt: &str,
    schema: Value,
    options: Option<&ModelOptions<'_>>,
) -> AppResult<String> {
    match provider {
        LlmProvider::CodexOauth => {
            return oauth_provider::request_structured_json(system_prompt, user_prompt, &schema, options).await;
        }
        LlmProvider::ClaudeOauth => {
            return claude_provider::request_structured_json(system_prompt, user_prompt, &schema, options).await;
        }
        LlmProvider::Litellm => {}
    }

    let base_url = std::env::var("MINUTES_LLM_BASE_URL")
        .unwrap_or_else(|_| DEFAULT_LITELLM_BASE_URL.to_string());
    let model = options.map_or_else(
        || std::env::var("MINUTES_LLM_MODEL").unwrap_or_else(|_| DEFAULT_LITELLM_MODEL.to_string()),
        |options| options.model.to_string(),
    );
    let endpoint = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()?;
    let mut body = json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt}
        ],
        "temperature": 0.1,
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "minutes_response",
                "strict": true,
                "schema": schema
            }
        }
    });
    apply_model_options(&mut body, ProviderType::OpenaiCompatible, options.or(Some(&ModelOptions { model: &model, reasoning_effort: None, fast: false })));
    let mut request = client.post(endpoint).json(&body);
    if let Ok(api_key) = std::env::var("MINUTES_LLM_API_KEY") {
        if !api_key.trim().is_empty() {
            request = request.bearer_auth(api_key);
        }
    }

    let completion: ChatCompletion = request.send().await?.error_for_status()?.json().await?;
    completion
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.message.content)
        .filter(|content| !content.trim().is_empty())
        .ok_or_else(|| AppError::InvalidState("LLM returned no response content".to_string()))
}

/// Routes a structured generation request by provider type, base_url, api_key, and model name.
/// This is the new dispatch path used by the model_assignments table. For built-in OAuth
/// providers (codex_oauth, claude_oauth), the caller should use the original
/// `request_structured_json` with the matching `LlmProvider` variant instead.
pub async fn request_structured_json_by_type(
    provider_type: ProviderType,
    base_url: &str,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    schema: &Value,
    reasoning_effort: Option<&str>,
    fast: bool,
) -> AppResult<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()?;
    let options = ModelOptions { model, reasoning_effort, fast };

    match provider_type {
        ProviderType::Openai | ProviderType::OpenaiCompatible => {
            let endpoint = if base_url.trim().is_empty() {
                "https://api.openai.com/v1/chat/completions".to_string()
            } else {
                format!(
                    "{}/chat/completions",
                    base_url.trim_end_matches('/').trim_end_matches("/v1")
                )
            };
            let mut body = json!({
                "model": model,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": user_prompt}
                ],
                "response_format": {
                    "type": "json_schema",
                    "json_schema": {
                        "name": "minutes_response",
                        "strict": true,
                        "schema": schema
                    }
                }
            });
            apply_model_options(&mut body, provider_type, Some(&options));
            let mut request = client.post(&endpoint).json(&body);
            if !api_key.trim().is_empty() {
                request = request.bearer_auth(api_key);
            }
            let completion: ChatCompletion =
                request.send().await?.error_for_status()?.json().await?;
            completion
                .choices
                .into_iter()
                .next()
                .map(|choice| choice.message.content)
                .filter(|content| !content.trim().is_empty())
                .ok_or_else(|| {
                    AppError::InvalidState("LLM returned no response content".to_string())
                })
        }
        ProviderType::Anthropic => {
            let endpoint = if base_url.trim().is_empty() {
                "https://api.anthropic.com/v1/messages".to_string()
            } else {
                format!("{}/v1/messages", base_url.trim_end_matches('/'))
            };
            let mut body = json!({
                    "model": model,
                    "system": system_prompt,
                    "messages": [{"role": "user", "content": user_prompt}],
                    "max_tokens": 4096,
                    "extra": {"response_format": {"type": "json_schema", "json_schema": {"name": "minutes_response", "schema": schema}}}
            });
            let fast_beta = apply_model_options(&mut body, provider_type, Some(&options));
            let mut request = client
                .post(&endpoint)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&body);
            if fast_beta { request = request.header("anthropic-beta", "fast-mode-2026-02-01"); }
            if !api_key.trim().is_empty() {
                request = request.header("x-api-key", api_key);
            }
            let raw = request.send().await?.error_for_status()?.text().await?;
            // Anthropic returns a slightly different shape; we expect content[0].text
            let response: Value = serde_json::from_str(&raw)
                .map_err(|e| AppError::InvalidState(format!("invalid anthropic response: {e}")))?;
            let content = response["content"]
                .as_array()
                .and_then(|arr| arr.iter().find(|block| block["type"] == "text"))
                .and_then(|block| block["text"].as_str())
                .ok_or_else(|| {
                    AppError::InvalidState("Anthropic response missing content[0].text".to_string())
                })?;
            Ok(content.to_string())
        }
    }
}

/// Resolved provider configuration used by the model_assignments dispatch path.
#[derive(Debug, Clone)]
pub struct ResolvedProvider {
    pub provider_type: ProviderType,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub reasoning_effort: Option<String>,
    pub fast: bool,
}

/// Generates minutes using a resolved provider from the model_assignments table
/// instead of the legacy LlmProvider enum.
pub async fn generate_minutes_with_resolved(
    resolved: ResolvedProvider,
    recording_id: Uuid,
    segments: &[TranscriptSegment],
    template: Option<&str>,
) -> AppResult<MinutesDraft> {
    let final_segments: Vec<&TranscriptSegment> = segments
        .iter()
        .filter(|segment| segment.is_final && !segment.text.trim().is_empty())
        .collect();
    if final_segments.is_empty() {
        return Err(AppError::InvalidState(
            "cannot generate minutes without final transcript segments".to_string(),
        ));
    }

    let transcript = final_segments
        .iter()
        .map(|segment| {
            json!({
                "id": segment.id,
                "start_ms": segment.start_ms,
                "end_ms": segment.end_ms,
                "speaker_label": segment.speaker_label,
                "text": segment.text,
            })
        })
        .collect::<Vec<_>>();
    let transcript_json = serde_json::to_string(&transcript).map_err(|error| {
        AppError::InvalidState(format!("failed to serialize transcript for LLM: {error}"))
    })?;

    let system_prompt = minutes_system_prompt(template);
    let user_prompt = format!(
        "다음 전사 세그먼트로 한국어 회의록을 작성하세요. 각 결정과 할 일에 직접 관련된 세그먼트 id를 인용하세요.\n\n{transcript_json}"
    );

    let content = request_structured_json_by_type(
        resolved.provider_type,
        &resolved.base_url,
        &resolved.api_key,
        &resolved.model,
        &system_prompt,
        &user_prompt,
        &minutes_schema(),
        resolved.reasoning_effort.as_deref(), resolved.fast,
    )
    .await?;
    let valid_segment_ids = final_segments
        .iter()
        .map(|segment| segment.id)
        .collect::<Vec<_>>();
    parse_minutes_response(recording_id, &content, &valid_segment_ids)
}

/// Edits a single minutes item using a resolved provider from the model_assignments table.
pub async fn edit_minutes_item_text_with_resolved(
    resolved: ResolvedProvider,
    item: &MinutesItem,
    instruction: &str,
    evidence_segments: &[TranscriptSegment],
) -> AppResult<String> {
    let instruction = instruction.trim();
    if instruction.is_empty() {
        return Err(AppError::InvalidState(
            "minutes edit instruction cannot be empty".to_string(),
        ));
    }
    if evidence_segments.is_empty() {
        return Err(AppError::InvalidState(format!(
            "minutes item {} has no available evidence segments",
            item.id
        )));
    }

    let evidence = evidence_segments
        .iter()
        .map(|segment| {
            json!({
                "id": segment.id,
                "speaker_label": segment.speaker_label,
                "text": segment.text,
            })
        })
        .collect::<Vec<_>>();
    let user_prompt = serde_json::to_string(&json!({
        "current_text": item.text,
        "instruction": instruction,
        "evidence": evidence,
    }))
    .map_err(|error| {
        AppError::InvalidState(format!("failed to serialize minutes edit request: {error}"))
    })?;

    let system_prompt = concat!(
        "당신은 한국어 회의록의 항목 하나만 수정합니다. 사용자 지시를 따르되 evidence에 없는 사실을 추가하지 마세요. ",
        "다른 항목, id, evidence_segment_ids는 수정할 수 없습니다. 수정된 한국어 text 하나만 JSON 스키마에 맞춰 반환하세요."
    );
    let content = request_structured_json_by_type(
        resolved.provider_type,
        &resolved.base_url,
        &resolved.api_key,
        &resolved.model,
        system_prompt,
        &user_prompt,
        &edit_schema(),
        resolved.reasoning_effort.as_deref(), resolved.fast,
    )
    .await?;
    let edited: EditedText = serde_json::from_str(&content).map_err(|error| {
        AppError::InvalidState(format!("LLM returned invalid minutes edit JSON: {error}"))
    })?;
    let text = edited.text.trim().to_string();
    if text.is_empty() {
        return Err(AppError::InvalidState(
            "LLM returned empty text for minutes edit".to_string(),
        ));
    }
    Ok(text)
}

// ------------------------------------------------------------------
// Grounded Q&A over one recording
// ------------------------------------------------------------------

const ASK_SYSTEM_PROMPT: &str = concat!(
    "당신은 특정 회의 기록에 대해서만 답하는 한국어 어시스턴트입니다. 제공된 전사와 회의록에 있는 내용만 사용하세요. ",
    "근거가 없으면 추측하지 말고 기록에 없다고 답하세요. ",
    "답변을 뒷받침하는 전사 세그먼트의 id를 source_segment_ids에 원문 그대로 인용하세요. ",
    "JSON 스키마 이외의 텍스트를 출력하지 마세요."
);

fn ask_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["answer", "source_segment_ids"],
        "properties": {
            "answer": {"type": "string", "minLength": 1},
            "source_segment_ids": {
                "type": "array",
                "items": {"type": "string", "format": "uuid"}
            }
        }
    })
}

/// Builds the user prompt for [`ask_note`]: the question plus the whole grounding context.
pub fn ask_user_prompt(
    question: &str,
    segments: &[TranscriptSegment],
    minutes: Option<&MinutesDraft>,
) -> AppResult<String> {
    let transcript = segments
        .iter()
        .filter(|segment| segment.is_final && !segment.text.trim().is_empty())
        .map(|segment| {
            json!({
                "id": segment.id,
                "start_ms": segment.start_ms,
                "end_ms": segment.end_ms,
                "speaker_label": segment.speaker_label,
                "text": segment.text,
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&json!({
        "question": question,
        "transcript": transcript,
        "minutes": minutes.map(|draft| json!({
            "summary": draft.summary,
            "decisions": draft.decisions.iter().map(|item| &item.text).collect::<Vec<_>>(),
            "action_items": draft.action_items.iter().map(|item| &item.text).collect::<Vec<_>>(),
        })),
    }))
    .map_err(|error| AppError::InvalidState(format!("failed to serialize ask request: {error}")))
}

/// Best-effort parse of the model's answer. Citations the model invented, malformed, or omitted
/// are dropped silently — a usable answer with no sources beats an error the user cannot act on.
pub fn parse_ask_response(response: &str, segments: &[TranscriptSegment]) -> AskAnswer {
    let parsed: Option<Value> = serde_json::from_str(response).ok();
    let answer = parsed
        .as_ref()
        .and_then(|value| value["answer"].as_str())
        .map(str::trim)
        .filter(|answer| !answer.is_empty())
        .map(str::to_string)
        // The provider may return prose instead of the requested JSON; that prose IS the answer.
        .unwrap_or_else(|| response.trim().to_string());

    let cited: HashSet<Uuid> = parsed
        .as_ref()
        .and_then(|value| value["source_segment_ids"].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|id| id.as_str().and_then(|id| Uuid::parse_str(id).ok()))
        .collect();
    let sources = segments
        .iter()
        .filter(|segment| cited.contains(&segment.id))
        .map(|segment| AskSource {
            segment_id: segment.id,
            start_ms: segment.start_ms,
            end_ms: segment.end_ms,
        })
        .collect();

    AskAnswer { answer, sources }
}

/// Answers a question about one recording through the built-in OAuth/LiteLLM provider path.
pub async fn ask_note(
    provider: LlmProvider,
    question: &str,
    segments: &[TranscriptSegment],
    minutes: Option<&MinutesDraft>,
    options: Option<&ModelOptions<'_>>,
) -> AppResult<AskAnswer> {
    let user_prompt = ask_question_prompt(question, segments, minutes)?;
    let content =
        request_structured_json(provider, ASK_SYSTEM_PROMPT, &user_prompt, ask_schema(), options).await?;
    Ok(parse_ask_response(&content, segments))
}

/// Same, through a provider resolved from the model_assignments table.
pub async fn ask_note_with_resolved(
    resolved: ResolvedProvider,
    question: &str,
    segments: &[TranscriptSegment],
    minutes: Option<&MinutesDraft>,
) -> AppResult<AskAnswer> {
    let user_prompt = ask_question_prompt(question, segments, minutes)?;
    let content = request_structured_json_by_type(
        resolved.provider_type,
        &resolved.base_url,
        &resolved.api_key,
        &resolved.model,
        ASK_SYSTEM_PROMPT,
        &user_prompt,
        &ask_schema(),
        resolved.reasoning_effort.as_deref(), resolved.fast,
    )
    .await?;
    Ok(parse_ask_response(&content, segments))
}

fn ask_question_prompt(
    question: &str,
    segments: &[TranscriptSegment],
    minutes: Option<&MinutesDraft>,
) -> AppResult<String> {
    let question = question.trim();
    if question.is_empty() {
        return Err(AppError::InvalidState(
            "question cannot be empty".to_string(),
        ));
    }
    if segments.is_empty() {
        return Err(AppError::InvalidState(
            "cannot answer questions without a transcript".to_string(),
        ));
    }
    ask_user_prompt(question, segments, minutes)
}

/// The minutes system prompt, with an optional user template appended. The template is the
/// single prompt-customization point: it constrains style/sections but never relaxes the
/// evidence-grounding contract above it.
pub fn minutes_system_prompt(template: Option<&str>) -> String {
    const BASE: &str = concat!(
        "당신은 한국어 회의록 작성자입니다. 제공된 전사에 명시된 사실만 사용해 간결한 한국어 요약, 결정, 할 일을 작성하세요. ",
        "명시적으로 합의된 내용만 decisions에 넣고, 실제로 요청되거나 약속된 업무만 action_items에 넣으세요. ",
        "모든 decision/action_item은 최소 1개의 evidence_segment_ids를 가져야 한다. ",
        "evidence_segment_ids에는 해당 항목을 직접 뒷받침하는 입력 세그먼트의 id만 원문 그대로 넣으세요. ",
        "근거가 없는 항목은 만들지 마세요. JSON 스키마 이외의 텍스트를 출력하지 마세요."
    );
    match template.map(str::trim).filter(|t| !t.is_empty()) {
        Some(template) => format!(
            "{BASE}\n\n다음 사용자 템플릿의 형식과 항목 구성을 따르되, 위의 근거 규칙은 그대로 지키세요:\n{template}"
        ),
        None => BASE.to_string(),
    }
}

fn minutes_schema() -> Value {
    let item_schema = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["text", "evidence_segment_ids"],
        "properties": {
            "text": {"type": "string", "minLength": 1},
            "evidence_segment_ids": {
                "type": "array",
                "minItems": 1,
                "items": {"type": "string", "format": "uuid"}
            }
        }
    });
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["summary", "decisions", "action_items"],
        "properties": {
            "summary": {"type": "string", "minLength": 1},
            "decisions": {"type": "array", "items": item_schema.clone()},
            "action_items": {"type": "array", "items": item_schema}
        }
    })
}

fn edit_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["text"],
        "properties": {
            "text": {"type": "string", "minLength": 1}
        }
    })
}

/// Filters caption events to only COMMITTED or REVISED, mapping them to TranscriptSegment
/// so the existing minutes pipeline can consume them without interface changes.
pub fn filter_caption_segments_for_minutes(events: &[CaptionEvent]) -> Vec<TranscriptSegment> {
    events
        .iter()
        .filter(|ev| matches!(ev.status, CaptionStatus::Committed | CaptionStatus::Revised))
        .map(|ev| TranscriptSegment {
            id: ev.segment_id,
            recording_id: uuid::Uuid::default(),
            start_ms: ev.start_ms(),
            end_ms: ev.end_ms(),
            speaker_label: ev.speaker_label.clone().unwrap_or_default(),
            text: ev.text.clone(),
            is_final: true,
        })
        .collect()
}

#[cfg(test)]
mod caption_gate_tests {
    use super::*;
    use crate::models::{CaptionEvent, CaptionStatus};
    use uuid::Uuid;

    fn make_event(status: CaptionStatus) -> CaptionEvent {
        CaptionEvent {
            segment_id: Uuid::new_v4(),
            start_sample: 0,
            end_sample: 4800,
            text: "test".to_string(),
            status,
            speaker_label: None,
            overlap: None,
            supersedes: vec![],
        }
    }

    #[test]
    fn test_committed_segments_are_included() {
        let events = vec![
            make_event(CaptionStatus::Committed),
            make_event(CaptionStatus::Revised),
        ];
        let segments = filter_caption_segments_for_minutes(&events);
        assert_eq!(segments.len(), 2);
    }

    #[test]
    fn test_partial_and_stable_are_excluded() {
        let events = vec![
            make_event(CaptionStatus::Partial),
            make_event(CaptionStatus::Stable),
            make_event(CaptionStatus::Committed),
        ];
        let segments = filter_caption_segments_for_minutes(&events);
        assert_eq!(segments.len(), 1);
    }

    #[test]
    fn test_empty_input_returns_empty() {
        let segments = filter_caption_segments_for_minutes(&[]);
        assert!(segments.is_empty());
    }
}
