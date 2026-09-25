//! Snapshot flush, interrupted-turn finalization, and the partial-answer
//! carry-over into the session transcript.

use super::state::TurnStateMirror;
use crate::threads::turn_state::types::TurnLifecycle;

pub(super) const MIRROR_LOG_PREFIX: &str = "[threads:turn_state:mirror]";

impl TurnStateMirror {
    /// Mark the turn as `Interrupted` on the in-memory snapshot and
    /// flush. Called when the bridge exits without a `TurnCompleted`
    /// event (i.e. the agent loop errored out).
    pub fn finish(mut self) {
        if self.turn_completed {
            return;
        }
        // The turn driver settles this snapshot the moment the turn ends
        // (`TurnStateStore::settle_turn`), because this bridge can outlive its
        // turn by minutes — it only exits when its progress sender drops, and
        // for a cached per-thread session that waits for the *next* turn. If the
        // driver already recorded a terminal outcome, keep it: a late blind
        // `Interrupted` here would raise a retry banner on a turn that actually
        // succeeded and whose reply was delivered.
        let already_completed = matches!(
            self.store
                .get_turn(&self.state.thread_id, &self.state.request_id)
                .ok()
                .flatten()
                .map(|snapshot| snapshot.lifecycle),
            Some(TurnLifecycle::Completed)
        );
        self.state.lifecycle = if already_completed {
            self.state.phase = None;
            TurnLifecycle::Completed
        } else {
            TurnLifecycle::Interrupted
        };
        self.state.active_tool = None;
        self.state.active_subagent = None;
        self.state.updated_at = chrono::Utc::now().to_rfc3339();
        self.flush();
        // Only a genuinely interrupted turn has a partial answer to carry over;
        // a settled turn's reply was already delivered and persisted, and
        // appending it again would duplicate it in the session transcript.
        if !already_completed {
            self.persist_interrupted_partial();
        }
    }

    /// Append the partial streamed answer of an interrupted turn to the session
    /// transcript so the derived display view (Phase B) can surface it even
    /// after the live turn_state snapshot is gone. Display-only: the
    /// model-context reader skips `interrupted:true` lines.
    ///
    /// Guard: the root transcript file must already exist. An interrupted
    /// **first** turn has no session file yet (the harness has not persisted a
    /// turn), so there is nothing to append to — that case stays recoverable
    /// from the turn_state snapshot alone, as today. We log and skip it.
    fn persist_interrupted_partial(&self) {
        let partial = self.state.streaming_text.trim();
        if partial.is_empty() {
            return;
        }
        let thread_id = self.state.thread_id.trim();
        if thread_id.is_empty() {
            return;
        }
        let workspace_dir = self.store.workspace_dir();
        let Some(path) = tinyagents_session::transcript::find_root_transcript_for_thread(
            workspace_dir,
            thread_id,
        ) else {
            log::debug!(
                "{MIRROR_LOG_PREFIX} no root transcript for thread={thread_id} yet — leaving interrupted partial ({} chars) in turn_state snapshot only",
                partial.len()
            );
            return;
        };
        let request_id = if self.state.request_id.is_empty() {
            None
        } else {
            Some(self.state.request_id.as_str())
        };
        let thinking = self.state.thinking.trim();
        let reasoning = if thinking.is_empty() {
            None
        } else {
            Some(thinking)
        };
        match tinyagents_session::transcript::append_interrupted_partial(
            &path,
            partial,
            request_id,
            Some(self.state.iteration),
            reasoning,
        ) {
            Ok(()) => log::debug!(
                "{MIRROR_LOG_PREFIX} appended interrupted partial ({} chars, thinking={} chars) for thread={thread_id} request_id={} to {}",
                partial.len(),
                thinking.len(),
                self.state.request_id,
                path.display()
            ),
            Err(err) => log::warn!(
                "{MIRROR_LOG_PREFIX} failed to append interrupted partial for thread={thread_id}: {err}"
            ),
        }
    }

    pub(super) fn flush(&mut self) {
        if let Err(err) = self.store.put(&self.state) {
            log::warn!(
                "{MIRROR_LOG_PREFIX} failed to persist snapshot for thread={}: {err}",
                self.state.thread_id
            );
        }
    }
}
