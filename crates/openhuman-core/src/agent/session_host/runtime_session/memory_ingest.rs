//! Handing committed turns to memory's conversation ingestion.

use tinyagents_runtime::CommitReceipt;
use tinyinference_llm::message::Message;

use super::OpenHumanTurnPrelude;
use crate::agent::tinyagents::host::OpenHumanRunContext;

impl OpenHumanTurnPrelude {
    /// Hands a committed, user-authored, threaded turn to memory's
    /// conversation ingestion (`DomainEvent::ConversationTurnCommitted`).
    /// Tool calls travel by name and id only.
    pub(super) fn publish_committed_turn(&self, receipt: &CommitReceipt<OpenHumanRunContext>) {
        let user_text = self
            .mutable
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pending_user_text
            .take();
        let (Some(user_text), Some(thread_id)) = (user_text, self.thread_id.clone()) else {
            return;
        };
        let tool_calls = committed_tool_calls(&receipt.outcome.history);
        log::debug!(
            "[session_host] conversation turn committed tool_calls={}",
            tool_calls.len()
        );
        crate::core::bus::BUS.publish(
            crate::core::events::DomainEvent::ConversationTurnCommitted {
                thread_id,
                agent_id: Some(self.agent_definition_id.clone()).filter(|id| !id.trim().is_empty()),
                workspace: Some(self.action_dir.display().to_string()),
                channel: Some(self.event_channel.clone())
                    .filter(|channel| !channel.trim().is_empty()),
                user_text,
                assistant_text: receipt.outcome.output.clone().unwrap_or_default(),
                tool_calls,
                workspace_dir: self.workspace_dir.clone(),
            },
        );
    }
}

/// The tool calls of the last exchange in `history` (everything after the
/// final user message), by name and id.
fn committed_tool_calls(history: &[Message]) -> Vec<crate::core::events::ConversationToolCall> {
    let start = history
        .iter()
        .rposition(|message| matches!(message, Message::User(_)))
        .map_or(0, |index| index + 1);
    history[start..]
        .iter()
        .filter_map(|message| match message {
            Message::Assistant(assistant) => Some(&assistant.tool_calls),
            _ => None,
        })
        .flatten()
        .map(|call| crate::core::events::ConversationToolCall {
            name: call.name.clone(),
            id: Some(call.id.clone()).filter(|id| !id.is_empty()),
        })
        .collect()
}
