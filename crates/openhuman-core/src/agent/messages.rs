//! Durable OpenHuman transcript records.
//!
//! Runtime model calls use [`tinyinference_llm::message::Message`]. These
//! compact records preserve the stable JSONL/thread storage contract used by
//! existing installations and carry OpenHuman-only message metadata.

use serde::{Deserialize, Serialize};

use crate::inference::provider::ToolCall;

const TURN_USAGE_METADATA_KEY: &str = "openhuman_turn_usage";
const TOOL_FAILURE_METADATA_KEY: &str = "openhuman_tool_failure";
const REPLAYED_METADATA_KEY: &str = "openhuman_replayed";
const WRAPPED_VALUE_KEY: &str = "openhuman_wrapped_value";
const WRAPPED_FLAG: &str = "wrapped";

fn would_wrap(message: &ChatMessage) -> bool {
    matches!(&message.extra_metadata, Some(value) if !value.is_object())
}

fn insert_host_metadata(message: &mut ChatMessage, key: &str, value: serde_json::Value) {
    let mut map = match message.extra_metadata.take() {
        Some(serde_json::Value::Object(map)) => map,
        Some(existing) => {
            let mut map = serde_json::Map::new();
            map.insert(WRAPPED_VALUE_KEY.to_string(), existing);
            map
        }
        None => serde_json::Map::new(),
    };
    map.insert(key.to_string(), value);
    message.extra_metadata = Some(serde_json::Value::Object(map));
}

fn take_host_metadata(
    extra: &mut Option<serde_json::Value>,
    key: &str,
) -> Option<serde_json::Value> {
    let serde_json::Value::Object(map) = extra.as_mut()? else {
        return None;
    };
    let value = map.remove(key)?;
    let wrapped = value
        .get(WRAPPED_FLAG)
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if map.is_empty() {
        *extra = None;
    } else if wrapped {
        match [TOOL_FAILURE_METADATA_KEY, REPLAYED_METADATA_KEY]
            .into_iter()
            .find(|remaining| map.contains_key(*remaining))
        {
            Some(carrier) => {
                if let Some(serde_json::Value::Object(marker)) = map.get_mut(carrier) {
                    marker.insert(WRAPPED_FLAG.to_string(), serde_json::Value::Bool(true));
                }
            }
            None if map.len() == 1 => {
                if let Some(original) = map.remove(WRAPPED_VALUE_KEY) {
                    *extra = Some(original);
                }
            }
            None => {}
        }
    }
    Some(value)
}

/// Host-only compatibility marker for an execution failure that the provider
/// message dialect cannot represent. The durable session format itself uses
/// `TranscriptMessage::tool_failure`; this marker exists only while a row is
/// adapted through OpenHuman's `ChatMessage` runtime shape.
pub(crate) fn attach_chat_tool_failure_metadata(message: &mut ChatMessage, detail: Option<&str>) {
    let mut payload = serde_json::Map::new();
    payload.insert("failure".to_string(), serde_json::Value::Bool(true));
    if let Some(detail) = detail.map(str::trim).filter(|detail| !detail.is_empty()) {
        payload.insert(
            "detail".to_string(),
            serde_json::Value::String(detail.to_string()),
        );
    }
    if would_wrap(message) {
        payload.insert(WRAPPED_FLAG.to_string(), serde_json::Value::Bool(true));
    }
    insert_host_metadata(
        message,
        TOOL_FAILURE_METADATA_KEY,
        serde_json::Value::Object(payload),
    );
}

pub(crate) fn attach_chat_turn_usage_metadata(
    message: &mut ChatMessage,
    usage: &tinyagents_session::transcript::TurnUsage,
) {
    if let Ok(payload) = serde_json::to_value(usage) {
        insert_host_metadata(message, TURN_USAGE_METADATA_KEY, payload);
    }
}

/// Marks a host runtime message as coming from a previous durable row, so the
/// explicit adapter preserves its original correlation id on the next write.
pub(crate) fn mark_chat_replayed_if_unmarked(message: &mut ChatMessage) {
    if message
        .extra_metadata
        .as_ref()
        .and_then(|meta| meta.get(REPLAYED_METADATA_KEY))
        .is_some()
    {
        return;
    }
    let mut payload = serde_json::Map::new();
    payload.insert("request_id".to_string(), serde_json::Value::Null);
    if would_wrap(message) {
        payload.insert(WRAPPED_FLAG.to_string(), serde_json::Value::Bool(true));
    }
    insert_host_metadata(
        message,
        REPLAYED_METADATA_KEY,
        serde_json::Value::Object(payload),
    );
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    #[serde(default, skip_serializing)]
    pub id: Option<String>,
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing)]
    pub extra_metadata: Option<serde_json::Value>,
    /// Ascending byte offsets into [`Self::content`] at which the provider may
    /// place a prompt-cache breakpoint. Only meaningful on the system message.
    ///
    /// `skip_serializing` like `id` and `extra_metadata` above: these are a
    /// property of *this call*, derived from the freshly assembled prompt, and
    /// writing them into the JSONL transcript would persist offsets that stop
    /// matching the moment the prompt is rebuilt. `serde(default)` keeps every
    /// record already on disk loadable.
    #[serde(default, skip_serializing)]
    pub cache_breakpoints: Vec<usize>,
}

/// Convert the host's provider-facing record into the neutral durable
/// transcript record. This is intentionally a field-for-field conversion at
/// the host boundary: `tinyagents-session` must not know OpenHuman's runtime
/// message type, while no provider metadata may be discarded before durable
/// persistence.
pub(crate) fn transcript_message_from_chat(
    message: &ChatMessage,
) -> tinyagents_session::transcript::TranscriptMessage {
    let mut extra_metadata = message.extra_metadata.clone();
    let turn_usage = take_host_metadata(&mut extra_metadata, TURN_USAGE_METADATA_KEY)
        .and_then(|value| serde_json::from_value(value).ok());
    let tool_failure =
        take_host_metadata(&mut extra_metadata, TOOL_FAILURE_METADATA_KEY).map(|value| {
            tinyagents_session::transcript::ToolFailure {
                failed: true,
                detail: value
                    .get("detail")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string),
            }
        });
    let request_id =
        take_host_metadata(&mut extra_metadata, REPLAYED_METADATA_KEY).and_then(|value| {
            value
                .get("request_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        });
    tinyagents_session::transcript::TranscriptMessage {
        id: message.id.clone(),
        role: message.role.clone(),
        content: message.content.clone(),
        extra_metadata,
        cache_breakpoints: message.cache_breakpoints.clone(),
        turn_usage,
        request_id,
        interrupted: false,
        tool_failure,
    }
}

/// Convert a neutral durable transcript row back into the host's provider
/// message shape after the lossless transcript reader has completed replay.
pub(crate) fn chat_message_from_transcript(
    message: tinyagents_session::transcript::TranscriptMessage,
) -> ChatMessage {
    let mut chat = ChatMessage {
        id: message.id,
        role: message.role,
        content: message.content,
        extra_metadata: message.extra_metadata,
        cache_breakpoints: message.cache_breakpoints,
    };
    if let Some(usage) = message.turn_usage.as_ref() {
        attach_chat_turn_usage_metadata(&mut chat, usage);
    }
    if let Some(failure) = message
        .tool_failure
        .as_ref()
        .filter(|failure| failure.failed)
    {
        attach_chat_tool_failure_metadata(&mut chat, failure.detail.as_deref());
    }
    if message.request_id.is_some() {
        let mut payload = serde_json::Map::new();
        payload.insert(
            "request_id".to_string(),
            serde_json::Value::String(message.request_id.unwrap_or_default()),
        );
        if would_wrap(&chat) {
            payload.insert(WRAPPED_FLAG.to_string(), serde_json::Value::Bool(true));
        }
        insert_host_metadata(
            &mut chat,
            REPLAYED_METADATA_KEY,
            serde_json::Value::Object(payload),
        );
    }
    chat
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            id: None,
            role: "system".into(),
            content: content.into(),
            extra_metadata: None,
            cache_breakpoints: Vec::new(),
        }
    }

    /// A system message carrying prompt-cache breakpoints.
    ///
    /// `breakpoints` are ends-of-tier from
    /// [`crate::agent::prompts::SystemPromptBuilder::build_tiered`].
    /// Out-of-range or non-ascending offsets are dropped rather than trusted:
    /// a bad offset would split the prompt mid-sentence and the model would
    /// read the damage, whereas a dropped one costs only a cache miss.
    pub fn system_tiered(content: impl Into<String>, breakpoints: Vec<usize>) -> Self {
        let content = content.into();
        let mut previous = 0usize;
        let breakpoints: Vec<usize> = breakpoints
            .into_iter()
            .filter(|&offset| {
                let ok =
                    offset > previous && offset < content.len() && content.is_char_boundary(offset);
                if ok {
                    previous = offset;
                } else {
                    tracing::warn!(
                        offset,
                        len = content.len(),
                        "[prompts] dropping an invalid cache breakpoint"
                    );
                }
                ok
            })
            .collect();
        Self {
            id: None,
            role: "system".into(),
            content,
            extra_metadata: None,
            cache_breakpoints: breakpoints,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            id: None,
            role: "user".into(),
            content: content.into(),
            extra_metadata: None,
            cache_breakpoints: Vec::new(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            id: None,
            role: "assistant".into(),
            content: content.into(),
            extra_metadata: None,
            cache_breakpoints: Vec::new(),
        }
    }

    pub fn tool(content: impl Into<String>) -> Self {
        Self {
            id: None,
            role: "tool".into(),
            content: content.into(),
            extra_metadata: None,
            cache_breakpoints: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultMessage {
    pub tool_call_id: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ConversationMessage {
    Chat(ChatMessage),
    AssistantToolCalls {
        text: Option<String>,
        tool_calls: Vec<ToolCall>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reasoning_content: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        extra_metadata: Option<serde_json::Value>,
    },
    ToolResults(Vec<ToolResultMessage>),
}
