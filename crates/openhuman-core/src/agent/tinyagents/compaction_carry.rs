//! Carrying a turn's context compaction into the history the session persists.
//!
//! The harness compacts by folding older messages into a checkpoint inside the
//! run (`ContextCompressionMiddleware`) and reports the folded transcript on
//! `AgentRun::compacted_history`. The session driver, though, does not persist
//! the harness transcript verbatim: it rebuilds the turn's history from the
//! request plus the post-processed conversation (grounded close, required
//! output repair). So the fold is carried as *what* replaced the old messages
//! (the checkpoint) and *how much* of the tail survived (`kept_tail`
//! non-system messages), and re-applied to that rebuilt history.
//!
//! Persisting the compacted history makes the next turn start from the
//! checkpoint instead of re-reading and re-summarizing everything this turn
//! already folded. The runtime session sees a history that no longer extends
//! the persisted one, so it seals the current transcript generation and opens
//! the next: the full conversation stays on disk.

use tinyagents_harness::summarization::{find_safe_cutoff_point, is_checkpoint};
use tinyinference_llm::message::Message;

/// A turn's compaction, in a form that can be re-applied to the history the
/// session driver assembles.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CompactionCarry {
    /// The checkpoint message standing in for the folded history.
    pub checkpoint: Message,
    /// How many trailing non-system messages stay verbatim after it.
    pub kept_tail: usize,
}

impl CompactionCarry {
    /// The carry for a run's `compacted_history` (`None` when the run did not
    /// compact, or the history carries no checkpoint).
    pub(crate) fn from_compacted_history(compacted: &[Message]) -> Option<Self> {
        let at = compacted.iter().position(is_checkpoint)?;
        let kept_tail = compacted[at + 1..]
            .iter()
            .filter(|message| !matches!(message, Message::System(_)))
            .count();
        Some(Self {
            checkpoint: compacted[at].clone(),
            kept_tail,
        })
    }

    /// `history` with everything before its last `kept_tail` non-system
    /// messages folded into the checkpoint.
    ///
    /// Leading system messages stay first and the checkpoint follows them.
    /// System messages inside the folded range (mid-conversation notes) are
    /// kept; earlier checkpoints are dropped, since this one supersedes them.
    /// The cut is moved, if needed, so the kept tail never opens with a tool
    /// result whose call was folded. A history with nothing before the kept
    /// tail is returned unchanged.
    pub(crate) fn apply(&self, history: Vec<Message>) -> Vec<Message> {
        let lead = history
            .iter()
            .take_while(|message| matches!(message, Message::System(_)) && !is_checkpoint(message))
            .count();
        let body = &history[lead..];
        let non_system: Vec<usize> = body
            .iter()
            .enumerate()
            .filter(|(_, message)| !matches!(message, Message::System(_)))
            .map(|(index, _)| index)
            .collect();
        if self.kept_tail >= non_system.len() {
            tracing::debug!(
                kept_tail = self.kept_tail,
                messages = non_system.len(),
                "[compaction_carry] nothing to fold; persisting the history unchanged"
            );
            return history;
        }
        let requested = non_system[non_system.len() - self.kept_tail];
        let cut = find_safe_cutoff_point(body, requested);
        if cut != requested {
            tracing::debug!(
                requested,
                cut,
                "[compaction_carry] moved the cut to keep tool calls paired"
            );
        }

        let mut compacted = Vec::with_capacity(lead + 1 + body.len() - cut);
        compacted.extend(history[..lead].iter().cloned());
        compacted.push(self.checkpoint.clone());
        compacted.extend(
            body[..cut]
                .iter()
                .filter(|message| matches!(message, Message::System(_)) && !is_checkpoint(message))
                .cloned(),
        );
        compacted.extend(
            body[cut..]
                .iter()
                .filter(|message| !is_checkpoint(message))
                .cloned(),
        );
        tracing::info!(
            full = history.len(),
            compacted = compacted.len(),
            kept_tail = self.kept_tail,
            "[compaction_carry] persisting the compacted history"
        );
        compacted
    }
}

/// The text of the latest real user message in `history`, skipping
/// compaction checkpoints (which are user-role but not the user's words).
pub(crate) fn last_user_message(history: &[Message]) -> Option<&Message> {
    history
        .iter()
        .rev()
        .find(|message| matches!(message, Message::User(_)) && !is_checkpoint(message))
}

#[cfg(test)]
#[path = "compaction_carry_tests.rs"]
mod tests;
