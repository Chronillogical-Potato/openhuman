//! The frozen system-prompt prefix a session hands the runtime.
//!
//! The prompt is rendered once per session as cache tiers (see
//! `agent::prompts::TieredPrompt::system_messages`) and sent as one leading
//! system message per tier. These two helpers turn a rendered prompt into
//! that prefix and recover it from a resumed transcript, so `runtime_session`
//! never has to know how many messages a prefix is.

use tinyagents_runtime::PrefixSnapshot;
use tinyinference_llm::message::Message;

use crate::agent::prompts::TieredPrompt;

/// One system message per cache tier (stable+context, then volatile). The
/// harness gives each its own cacheable segment, so a rewritten memory file or
/// a newly connected service changes the second segment and leaves the first
/// byte-identical for the provider's prefix cache.
pub(super) fn tiered_prefix_snapshot(tiered: &TieredPrompt) -> PrefixSnapshot {
    let messages = tiered.system_messages();
    tracing::debug!(
        segments = messages.len(),
        bytes = ?messages.iter().map(String::len).collect::<Vec<_>>(),
        "[session] frozen system prompt as tiered segments"
    );
    PrefixSnapshot::new(messages.into_iter().map(Message::system).collect())
}

/// The frozen prefix of a resumed transcript: every leading system message,
/// not only the first, because the prompt is sent as one message per tier.
pub(super) fn leading_system_prefix(history: &[Message]) -> Option<PrefixSnapshot> {
    let leading: Vec<Message> = history
        .iter()
        .take_while(|message| matches!(message, Message::System(_)))
        .cloned()
        .collect();
    (!leading.is_empty()).then(|| PrefixSnapshot::new(leading))
}
