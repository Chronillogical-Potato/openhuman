//! Keeping what a failed chat turn did (#6281).

use crate::agent::harness::session::transcript::{MessageUsage, TurnUsage};
use crate::agent::harness::session::turn_checkpoint::{truncate_chars, CHECKPOINT_RESULT_CHARS};
use crate::agent::harness::session::types::Agent;
use crate::agent::harness::tool_result_artifacts::ToolResultArtifactStore;
use crate::agent::messages::{ChatMessage, ConversationMessage};
use crate::agent::tinyagents::{TranscriptSnapshot, TranscriptSnapshotSink};
use anyhow::Result;
use tinyinference::message::Message;

/// Leads the note a failed turn leaves in history, so the model (and a reader
/// of the transcript) can tell it from a reply.
const FAILED_TURN_NOTE_PREFIX: &str = "[turn failed before completion:";

impl Agent {
    /// Drive the chat turn and, if it fails, keep what it did.
    ///
    /// `turn()` has already pushed this turn's user message, but on `Err` the
    /// turn body returned before anything else it produced reached `history` or
    /// the transcript. Every follow-up ("what happened?", "try again") then ran
    /// without the tool calls, their results, or the reason the turn failed.
    ///
    /// On any error from the turn body (provider, harness, tool loop, or a typed
    /// error raised after the loop) this appends the rounds the provider had
    /// already accepted, then a failure note, and persists the transcript, so a
    /// warm follow-up and a cold-boot resume both see them. A turn future that is
    /// dropped (cancelled) is not an error and records nothing here.
    pub(super) async fn run_turn_via_tinyagents_session(
        &mut self,
        user_message: &str,
        effective_model: &str,
        temperature: f64,
        max_iterations: usize,
        artifact_store: Option<ToolResultArtifactStore>,
        suppress_tools: bool,
    ) -> Result<String> {
        let snapshot = TranscriptSnapshotSink::default();
        let result = Box::pin(self.run_turn_via_tinyagents_session_inner(
            user_message,
            effective_model,
            temperature,
            max_iterations,
            artifact_store,
            suppress_tools,
            snapshot.clone(),
        ))
        .await;
        if let Err(err) = &result {
            let snapshot = std::mem::take(
                &mut *snapshot
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
            self.record_failed_turn(&snapshot, effective_model, err);
        }
        result
    }

    fn record_failed_turn(
        &mut self,
        snapshot: &TranscriptSnapshot,
        effective_model: &str,
        err: &anyhow::Error,
    ) {
        let (accepted, unanswered) = split_snapshot(snapshot);
        log::warn!(
            "[agent_loop] turn failed; recording {} accepted round message(s), {} unanswered \
             message(s) as text, and the failure cause into history — #6281",
            accepted.len(),
            unanswered.len()
        );
        // Only rounds the provider already answered are replayed as structured
        // messages. The newest round was carried only by the failing request, and
        // a request rejected for malformed tool history must not be replayed, or
        // the de-poison eviction in `web_chat::run_task` would reseed the same
        // rejection from this transcript. That round goes into the note as text.
        self.history
            .extend(crate::agent::message_convert::messages_to_conversation(
                accepted,
            ));
        self.history
            .push(ConversationMessage::Chat(ChatMessage::assistant(
                failed_turn_note(err, unanswered),
            )));
        self.trim_history();

        let persisted = self.tool_dispatcher.to_provider_messages(&self.history);
        // A failed run reports no usage; record zeros, but keep the provider and
        // model so the transcript meta stays attributable.
        let turn_usage = TurnUsage {
            provider: self.event_channel().to_string(),
            model: effective_model.to_string(),
            usage: MessageUsage {
                input: 0,
                output: 0,
                cached_input: 0,
                context_window: 0,
                cost_usd: 0.0,
            },
            ts: chrono::Utc::now().to_rfc3339(),
            reasoning_content: None,
            tool_calls: Vec::new(),
            iteration: 0,
        };
        self.persist_session_transcript(&persisted, 0, 0, 0, 0.0, Some(&turn_usage));
    }
}

/// Split a snapshot into this run's rounds the provider answered and the ones
/// only the failing (unanswered) request carried.
fn split_snapshot(snapshot: &TranscriptSnapshot) -> (&[Message], &[Message]) {
    let len = snapshot.messages.len();
    let base = snapshot.request_base_len.min(len);
    let accepted_end = snapshot.accepted_len.clamp(base, len);
    (
        &snapshot.messages[base..accepted_end],
        &snapshot.messages[accepted_end..],
    )
}

/// The failure cause, plus the unanswered steps rendered as text.
fn failed_turn_note(err: &anyhow::Error, unanswered: &[Message]) -> String {
    let mut note = format!(
        "{FAILED_TURN_NOTE_PREFIX} {}]",
        truncate_chars(&err.to_string(), CHECKPOINT_RESULT_CHARS)
    );
    if unanswered.is_empty() {
        return note;
    }
    note.push_str("\n\nThe request that failed also carried these steps, recorded here as text:\n");
    for msg in unanswered {
        match msg {
            Message::Assistant(assistant) if !assistant.tool_calls.is_empty() => {
                for call in &assistant.tool_calls {
                    let call = crate::agent::message_convert::ta_call_to_oh_call(call);
                    note.push_str(&format!(
                        "- called `{}` with {}\n",
                        call.name,
                        truncate_chars(&call.arguments, CHECKPOINT_RESULT_CHARS)
                    ));
                }
            }
            Message::Tool(_) => note.push_str(&format!(
                "- tool result: {}\n",
                truncate_chars(&msg.text(), CHECKPOINT_RESULT_CHARS)
            )),
            Message::Assistant(_) => note.push_str(&format!(
                "- assistant: {}\n",
                truncate_chars(&msg.text(), CHECKPOINT_RESULT_CHARS)
            )),
            Message::User(_) | Message::System(_) => note.push_str(&format!(
                "- message: {}\n",
                truncate_chars(&msg.text(), CHECKPOINT_RESULT_CHARS)
            )),
        }
    }
    note
}
