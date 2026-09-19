//! Shared types for the task dispatcher.

use crate::agent::todos::ops::BoardLocation;

/// Handle to an in-flight autonomous run, keyed by its session `thread_id`.
///
/// Autonomous runs are detached `tokio` tasks, not web-channel turns, so they
/// are invisible to the web channel's own in-flight registry — which is why the
/// chat **Cancel** button (which calls `channel_web_cancel`) couldn't stop them.
/// Registering the run's [`AbortHandle`](tokio::task::AbortHandle) in the
/// crate's [`ActiveRunRegistry`](tinyagents_graph::todos::dispatch::ActiveRunRegistry)
/// lets [`cancel_session`](super::registry::cancel_session) abort it from that
/// same cancel path.
///
/// The `context` the crate carries for us is the run's [`BoardLocation`], which
/// the canceller needs to write the card back to a terminal state.
pub(super) type ActiveRun = tinyagents_graph::todos::dispatch::ActiveRun<BoardLocation>;

/// A resolved executor: which built-in agent definition to build, an optional
/// system-prompt suffix carrying skill guidelines, and a label for
/// logs/telemetry.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ResolvedExecutor {
    pub(super) agent_id: String,
    pub(super) prompt_suffix: Option<String>,
    pub(super) label: String,
}

impl ResolvedExecutor {
    pub(super) fn default_agent() -> Self {
        Self {
            agent_id: "orchestrator".to_string(),
            prompt_suffix: None,
            label: "default".to_string(),
        }
    }
}

/// Outcome of a dispatch attempt.
#[derive(Debug)]
pub enum DispatchOutcome {
    /// The card was claimed and a detached autonomous run was spawned.
    Running { run_id: String },
    /// Plan approval is required; the card was parked at `awaiting_approval`
    /// and a `TaskPlanAwaitingApproval` event was emitted. No run was spawned.
    AwaitingApproval,
}
