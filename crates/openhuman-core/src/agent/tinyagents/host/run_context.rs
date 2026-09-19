//! OpenHuman's explicit, host-owned state for one agent run.
//!
//! `tinyagents_harness::context::RunContext` owns generic runtime mechanics.
//! This type owns the product values that used to be recovered from ambient
//! task-locals: approval origin, host progress, attachment and artifact scope,
//! parent dispatch state, and the handles a tool needs to continue a run.
//! The shared turn seam receives this as its live OpenHuman carrier. The live
//! middleware registry uses this type as the TinyAgents run context,
//! so every host seam receives the same explicit carrier rather than recovering
//! product state from a task-local.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc::Sender;

use crate::agent::harness::definition::SandboxMode;
use crate::agent::harness::fork_context::{AgentContextPreparedSource, ParentExecutionContext};
use crate::agent::harness::tool_result_artifacts::ToolResultArtifactIndexStore;
use crate::agent::harness::turn_dispatch_guard::TurnDispatchState;
use crate::agent::harness::turn_subagent_usage::SubagentUsageEntry;
use crate::agent::progress::AgentProgress;
use crate::agent::stop_hooks::StopHook;
use crate::agent::tinyagents::turn_outcome::ToolOutcomeSink;
use crate::agent::turn_origin::AgentTurnOrigin;
use tinyinference_llm::model::ResolvedModelRoute;

/// Explicit OpenHuman data carried by a top-level or child agent run.
///
/// Shared members (`Arc`s, cancellation and workspace descriptor) retain the
/// identity required by a recursive run tree. [`Self::child`] deliberately
/// allocates a fresh route slot and subagent ledger: a child must not overwrite
/// the provider route or cost roll-up subsequently read by its parent.
#[derive(Clone)]
pub struct OpenHumanRunContext {
    /// Trust/routing source used by OpenHuman approval and attribution policy.
    pub origin: Option<AgentTurnOrigin>,
    /// UI/event progress receiver for this turn tree.
    pub progress: Option<Sender<AgentProgress>>,
    /// Stop policies evaluated after each model call.
    pub stop_hooks: Vec<Arc<dyn StopHook>>,
    /// Parent runtime snapshot used by canonical recursive tool dispatch.
    pub parent: Option<ParentExecutionContext>,
    /// Context-preparation sources already consumed in this turn.
    pub prepared_context_sources: Arc<Vec<AgentContextPreparedSource>>,
    /// File-state identity used to detect stale parent reads after child writes.
    pub file_state_agent_id: Option<String>,
    /// Host-owned artifact index for tool-result references.
    pub(crate) tool_result_artifact_index: Option<Arc<ToolResultArtifactIndexStore>>,
    /// Current-turn attachment placeholders forwarded to vision delegates.
    pub attachment_placeholders: Arc<Vec<String>>,
    /// Dispatch guard shared by synchronous delegates in this run.
    pub dispatch: Option<Arc<TurnDispatchState>>,
    /// Optional task recency restriction for integration tools.
    pub task_recency_window: Option<Duration>,
    /// Sandbox mode of the agent definition executing this turn.
    pub sandbox_mode: Option<SandboxMode>,
    /// Zero-based OpenHuman subagent depth (the root is zero).
    pub spawn_depth: usize,
    /// This run's subagent usage roll-up; intentionally isolated for children.
    pub subagent_usage: Arc<Mutex<Vec<SubagentUsageEntry>>>,
    /// Provider/model/host-route observation for this run, written from the
    /// canonical response metadata by typed model middleware. Intentionally
    /// isolated for children.
    pub(crate) resolved_route: Arc<Mutex<Option<ResolvedModelRoute>>>,
    /// Cooperative cancellation shared by the complete recursive run tree.
    pub cancellation: tinyagents_harness::cancel::CancellationToken,
    /// Thread attached to provider requests and host persistence.
    pub thread_id: Option<String>,
    /// Direct canonical workspace descriptor; never use the old harness re-export.
    pub workspace: Option<tinytools::WorkspaceDescriptor>,
    /// Per-turn tool result capture shared with the event bridge.
    pub(crate) tool_outcomes: Option<ToolOutcomeSink>,
}

impl Default for OpenHumanRunContext {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenHumanRunContext {
    /// Snapshot the values already scoped by a live legacy entrypoint.
    ///
    /// This is the transition boundary: roots construct one owned carrier from
    /// their current scopes, then recursive paths pass [`Self::child`] instead
    /// of depending on task-local inheritance. Explicit root inputs overwrite
    /// these snapshots only when they intentionally own the same value.
    pub(crate) fn from_current_scopes() -> Self {
        let mut context = Self::new();
        context.origin = crate::agent::turn_origin::current();
        context.progress = crate::agent::progress_sink::current_progress_sink();
        context.stop_hooks = crate::agent::stop_hooks::current_stop_hooks();
        context.dispatch = crate::agent::harness::turn_dispatch_guard::current();
        context.thread_id = crate::agent::tinyagents::thread_context::current_thread_id();
        context.workspace = crate::agent::turn_workspace::current()
            .filter(|root| root.is_dir())
            .map(|root| tinytools::WorkspaceDescriptor::new(root).with_policy_id("turn-workspace"));
        context
    }

    /// Builds an unbound context. Entry points set only the values their turn
    /// actually owns; `None` is an explicit absence, not an ambient fallback.
    pub fn new() -> Self {
        Self {
            origin: None,
            progress: None,
            stop_hooks: Vec::new(),
            parent: None,
            prepared_context_sources: Arc::new(Vec::new()),
            file_state_agent_id: None,
            tool_result_artifact_index: None,
            attachment_placeholders: Arc::new(Vec::new()),
            dispatch: None,
            task_recency_window: None,
            sandbox_mode: None,
            spawn_depth: 0,
            subagent_usage: Arc::new(Mutex::new(Vec::new())),
            resolved_route: Arc::new(Mutex::new(None)),
            cancellation: tinyagents_harness::cancel::CancellationToken::new(),
            thread_id: None,
            workspace: None,
            tool_outcomes: None,
        }
    }

    /// Sets the direct TinyTools workspace descriptor for this run.
    pub fn with_workspace(mut self, workspace: tinytools::WorkspaceDescriptor) -> Self {
        self.workspace = Some(workspace);
        self
    }

    /// Sets the same cancellation token on this context and its TinyAgents run.
    pub fn with_cancellation(
        mut self,
        cancellation: tinyagents_harness::cancel::CancellationToken,
    ) -> Self {
        self.cancellation = cancellation;
        self
    }

    /// Builds the explicit child state for a recursive invocation.
    ///
    /// This inheritance rule is tested in B1 and is ready for the later live
    /// plumbing. Cancellation, workspace, progress, policy hooks and dispatch
    /// state are inherited. Route observation and usage accounting are isolated,
    /// so a completed child cannot mutate facts subsequently persisted for its
    /// parent.
    pub fn child(&self) -> Self {
        let mut child = self.clone();
        child.spawn_depth = self.spawn_depth.saturating_add(1);
        child.file_state_agent_id = None;
        child.subagent_usage = Arc::new(Mutex::new(Vec::new()));
        child.resolved_route = Arc::new(Mutex::new(None));
        child
    }

    /// Converts this host context into TinyAgents' canonical run context.
    ///
    /// The canonical context receives the exact same cancellation and direct
    /// `tinytools::WorkspaceDescriptor`; no type alias, re-export or task-local
    /// bridge is involved.
    pub fn into_tinyagents(
        self,
        mut config: tinyagents_harness::context::RunConfig,
    ) -> tinyagents_harness::context::RunContext<Self> {
        if config.thread_id.is_none() {
            if let Some(thread_id) = self.thread_id.as_deref() {
                config = config.with_thread(thread_id);
            }
        }
        let cancellation = self.cancellation.clone();
        let workspace = self.workspace.clone();
        let context = tinyagents_harness::context::RunContext::new(config, self)
            .with_cancellation(cancellation);
        match workspace {
            Some(workspace) => context.with_workspace(workspace),
            None => context,
        }
    }

    /// Returns this context's file-state identity for explicit tool plumbing.
    pub fn file_state_scope(&self) -> Option<&str> {
        self.file_state_agent_id.as_deref()
    }

    /// Records a child usage entry without relying on a task-local collector.
    pub fn append_subagent_usage(&self, entry: SubagentUsageEntry) {
        self.subagent_usage
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(entry);
    }

    /// Resolves whether a file-state scope has been assigned to this context.
    /// Keeping the conversion here makes file-state callers use the explicit
    /// context instead of discovering an ambient identifier.
    pub fn file_state_agent_id(&self) -> Option<String> {
        self.file_state_agent_id.clone()
    }
}

#[cfg(test)]
#[path = "run_context_tests.rs"]
mod tests;
