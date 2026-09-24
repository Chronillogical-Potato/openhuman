//! `threads.edit_message` / `threads.regenerate`: cancel the thread's
//! in-flight turn, fork the session transcript at a cut point (the sealed
//! generation it forks from is never touched — same guarantee a compaction
//! gives), trim the conversation-store message log to match, drop the
//! turn-state snapshots for every turn the fork drops, then restart the turn.
//!
//! ## Mapping a UI message id to a transcript cut point
//!
//! The frontend only has ids from `threads.messages_list`
//! (`ConversationMessageRecord.id`) — a different id space from the
//! model-facing transcript, which keys a turn's rows by
//! `TranscriptMessage::request_id`. The one place these two id spaces
//! provably correlate is an **assistant reply**: its store id is minted
//! deterministically as `agent:<request_id>`
//! ([`crate::memory::conversations::run_reply_message_id`], written by
//! `web_chat::reply_persistence` before the `chat_done` that announces it),
//! so stripping that prefix recovers the exact turn id the transcript
//! recorded on every row of that turn.
//!
//! - `regenerate { message_id: Some(id) }` — `id` must be that deterministic
//!   reply id. Its `request_id` is the turn to redo: the transcript is cut
//!   before that turn's first row (dropping the stored answer and
//!   everything after, keeping the user prompt that produced it), and the
//!   message log is truncated from that same reply's store id onward.
//! - `regenerate { message_id: None }` — redo the thread's last turn
//!   ([`tinyagents_session::transcript::TruncateCut::LastAssistantTurn`]),
//!   no id correlation needed.
//! - `edit_message { message_id }` — `message_id` names the **user** message
//!   being edited, which carries no such correlation (the frontend mints it
//!   optimistically, before the server has picked a `request_id`). Instead,
//!   this resolves through the *next* deterministic reply id after it in the
//!   store's own message order, recovers that turn's `request_id`, and cuts
//!   the transcript before that turn's first row — the same point
//!   `regenerate` would cut for that turn, since editing a prompt discards
//!   the answer it produced exactly the way redoing it does. A user message
//!   with no reply yet (editing the newest, still-unanswered message) has
//!   nothing on the model side to cut; only the message-log tail is
//!   truncated in that case, and the edit still lands as a fresh turn.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use tinyagents_session::transcript::{
    self, FileTranscriptLocator, SessionRef, SessionTranscript, TranscriptLocator, TruncateCut,
};

use crate::memory::conversations::{is_deterministic_message_id, run_reply_message_id};
use crate::rpc::RpcOutcome;
use crate::threads::ThreadsError;

use super::support::workspace_dir;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditMessageRequest {
    pub thread_id: String,
    pub message_id: String,
    pub content: String,
    #[serde(default)]
    pub client_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegenerateRequest {
    pub thread_id: String,
    #[serde(default)]
    pub message_id: Option<String>,
    #[serde(default)]
    pub client_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EditOrRegenerateResponse {
    pub request_id: String,
}

/// Edit a past user message: cancel the in-flight turn (if any), fork the
/// session transcript and message log to drop that message and everything
/// after it, then restart the turn with `content` in its place.
pub async fn edit_message(
    request: EditMessageRequest,
) -> Result<RpcOutcome<Value>, ThreadsError> {
    let client_id = request.client_id.unwrap_or_else(|| "system".to_string());
    let thread_id = request.thread_id;
    let dir = workspace_dir().await?;

    crate::web_chat::cancel_chat(&client_id, &thread_id)
        .await
        .map_err(ThreadsError::Message)?;

    // Correlate through the next assistant reply, if any — see the module
    // doc's mapping section. `None` means the edited message has no reply
    // yet, so there is nothing to cut on the model side.
    let cut_request_id = next_reply_request_id_after(&dir, &thread_id, &request.message_id)
        .await
        .map_err(ThreadsError::Message)?;

    if let Some(cut_request_id) = &cut_request_id {
        truncate_transcript(&dir, &thread_id, cut_request_id)
            .map_err(ThreadsError::Message)?;
        clear_dropped_turn_states(&dir, &thread_id, cut_request_id);
    }

    // Truncate the message log at the edited message itself (inclusive) —
    // it and everything after it is replaced by the fresh turn below.
    conversations_delete_after(&dir, &thread_id, &request.message_id)
        .await
        .map_err(ThreadsError::Message)?;

    crate::web_chat::invalidate_thread_sessions(&thread_id).await;

    let new_request_id = restart_turn(&client_id, &thread_id, &request.content)
        .await
        .map_err(ThreadsError::Message)?;

    Ok(RpcOutcome::single_log(
        json!(EditOrRegenerateResponse {
            request_id: new_request_id,
        }),
        "message edited, turn restarted",
    ))
}

/// Regenerate a past assistant reply (or, with no `message_id`, the thread's
/// last turn): cancel the in-flight turn (if any), fork the session
/// transcript and message log to drop the answer and everything after it,
/// then restart the turn with the same user prompt that produced it.
pub async fn regenerate(request: RegenerateRequest) -> Result<RpcOutcome<Value>, ThreadsError> {
    let client_id = request.client_id.unwrap_or_else(|| "system".to_string());
    let thread_id = request.thread_id;
    let dir = workspace_dir().await?;

    crate::web_chat::cancel_chat(&client_id, &thread_id)
        .await
        .map_err(ThreadsError::Message)?;

    let cut = match &request.message_id {
        Some(message_id) => {
            let request_id = reply_request_id(message_id).ok_or_else(|| {
                ThreadsError::Message(format!(
                    "message {message_id} is not a regenerable assistant reply"
                ))
            })?;
            TruncateCut::BeforeIndex(0).placeholder_unused(); // silence unused import lints below if any
            RegenerateCut::Turn(request_id)
        }
        None => RegenerateCut::LastTurn,
    };

    let (prompt, cut_request_id) = match &cut {
        RegenerateCut::Turn(request_id) => {
            truncate_transcript(&dir, &thread_id, request_id).map_err(ThreadsError::Message)?;
            let prompt = user_prompt_for_turn(&dir, &thread_id, request_id)
                .map_err(ThreadsError::Message)?
                .ok_or_else(|| {
                    ThreadsError::Message(format!(
                        "no user prompt found for turn {request_id} in thread {thread_id}"
                    ))
                })?;
            (prompt, Some(request_id.clone()))
        }
        RegenerateCut::LastTurn => {
            let prompt = truncate_transcript_last_turn(&dir, &thread_id)
                .map_err(ThreadsError::Message)?
                .ok_or_else(|| {
                    ThreadsError::Message(format!(
                        "thread {thread_id} has no turn to regenerate"
                    ))
                })?;
            (prompt, None)
        }
    };

    if let Some(cut_request_id) = &cut_request_id {
        clear_dropped_turn_states(&dir, &thread_id, cut_request_id);
        conversations_delete_after(&dir, &thread_id, &run_reply_message_id(cut_request_id))
            .await
            .map_err(ThreadsError::Message)?;
    }

    crate::web_chat::invalidate_thread_sessions(&thread_id).await;

    let new_request_id = restart_turn(&client_id, &thread_id, &prompt)
        .await
        .map_err(ThreadsError::Message)?;

    Ok(RpcOutcome::single_log(
        json!(EditOrRegenerateResponse {
            request_id: new_request_id,
        }),
        "turn regenerated",
    ))
}

enum RegenerateCut {
    Turn(String),
    LastTurn,
}

/// The `request_id` a deterministic assistant-reply store id was minted for,
/// or `None` if `id` is not one (see [`is_deterministic_message_id`]).
fn reply_request_id(id: &str) -> Option<String> {
    is_deterministic_message_id(id).then(|| {
        id.trim_start_matches(crate::memory::conversations::store_types::DETERMINISTIC_MESSAGE_ID_PREFIX)
            .to_string()
    })
}

async fn conversations_delete_after(
    dir: &std::path::Path,
    thread_id: &str,
    message_id: &str,
) -> Result<(), String> {
    super::delete_after(thread_id, message_id)
        .await
        .map_err(|e| e.to_string())?;
    let _ = dir;
    Ok(())
}

async fn restart_turn(client_id: &str, thread_id: &str, content: &str) -> Result<String, String> {
    crate::web_chat::start_chat(
        client_id,
        thread_id,
        content,
        None,
        None,
        None,
        None,
        crate::web_chat::ChatRequestMetadata::default(),
    )
    .await
    .map_err(|e| e.to_string())
}
