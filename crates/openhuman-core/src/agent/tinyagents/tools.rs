//! `tinyagents` [`Tool`] adapter over an openhuman [`Tool`] (issue #4249).
//!
//! It resolves a tool from the session's shared registry and delegates through
//! the canonical `tinytools::Tool` contract without creating a second tool
//! result vocabulary.

use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use tinyagents_harness::steering::{SteeringCommand, SteeringHandle};
use tinytools::{Tool, ToolCallOptions, ToolResult, ToolRunContext};

/// A captured early-exit: a sub-agent invoked an early-exit tool (e.g.
/// `ask_user_clarification`), so the loop should pause and surface `question`
/// to the user. Mirrors the legacy `run_turn_engine` `early_exit_tool` seam.
#[derive(Debug, Clone)]
pub(crate) struct EarlyExit {
    pub(crate) tool: String,
    pub(crate) question: String,
}

/// Shared early-exit hook handed to the adapters for the early-exit tool names.
/// On a successful call to one of those tools it records the [`EarlyExit`] and
/// sends a [`SteeringCommand::Pause`] so the harness loop short-circuits at the
/// next checkpoint (before the next model call) — the tinyagents analogue of the
/// legacy loop's "break on early-exit tool" behavior.
#[derive(Clone)]
pub(crate) struct EarlyExitHook {
    handle: SteeringHandle,
    slot: Arc<Mutex<Option<EarlyExit>>>,
}

impl EarlyExitHook {
    /// Build a hook that pauses `handle` and records into a fresh slot.
    pub(crate) fn new(handle: SteeringHandle) -> Self {
        Self {
            handle,
            slot: Arc::new(Mutex::new(None)),
        }
    }

    /// The captured early-exit, if one fired during the run.
    pub(crate) fn take(&self) -> Option<EarlyExit> {
        // #4469 item 3: recover a poisoned slot rather than panic — a panic while
        // some other tool held this lock must not swallow the early-exit.
        self.slot
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }

    /// Record an early-exit and request a cooperative pause. Only the first
    /// early-exit in a run is kept (matching the legacy "halt on first").
    fn trigger(&self, tool: &str, question: String) {
        {
            // #4469 item 3: `into_inner` keeps early-exit recording working even
            // if the slot mutex was poisoned by an unrelated panic.
            let mut slot = self.slot.lock().unwrap_or_else(PoisonError::into_inner);
            if slot.is_none() {
                *slot = Some(EarlyExit {
                    tool: tool.to_string(),
                    question,
                });
            }
        }
        tracing::info!(tool, "[tinyagents] early-exit tool — requesting pause");
        self.handle.send(SteeringCommand::Pause);
    }
}

/// Canonical TinyTools adapter over the shared session tool sets.
///
/// The runtime now executes the canonical `tinytools::Tool` contract directly;
/// this wrapper only resolves a non-cloneable tool from the shared registry and
/// preserves the early-exit control hook.
pub(crate) struct CanonicalSharedToolAdapter {
    sets: Vec<Arc<Vec<Box<dyn Tool>>>>,
    name: String,
    description: String,
    parameters_schema: serde_json::Value,
    early_exit: Option<EarlyExitHook>,
}

impl CanonicalSharedToolAdapter {
    pub(crate) fn for_name(sets: Vec<Arc<Vec<Box<dyn Tool>>>>, name: &str) -> Option<Self> {
        let spec = sets
            .iter()
            .flat_map(|set| set.iter())
            .find(|tool| tool.name() == name)
            .map(|tool| tool.spec())?;
        Some(Self {
            sets,
            name: spec.name,
            description: spec.description,
            parameters_schema: spec.parameters,
            early_exit: None,
        })
    }

    pub(crate) fn with_early_exit(mut self, hook: EarlyExitHook) -> Self {
        self.early_exit = Some(hook);
        self
    }

    fn resolved_tool(&self) -> Option<&dyn Tool> {
        self.sets
            .iter()
            .flat_map(|set| set.iter())
            .find(|tool| tool.name() == self.name)
            .map(|tool| tool.as_ref())
    }
}

#[async_trait]
impl Tool for CanonicalSharedToolAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters_schema(&self) -> serde_json::Value {
        self.parameters_schema.clone()
    }

    fn policy(&self) -> tinytools::ToolPolicy {
        self.resolved_tool().map(Tool::policy).unwrap_or_default()
    }

    /// Forwarded so the harness advertises only `Direct` registrations and
    /// indexes `Deferred` ones for its `tool_search` bridge. Without this
    /// every registered tool reported `Direct` and the bridge stayed inert.
    fn exposure(&self) -> tinytools::ToolExposure {
        self.resolved_tool()
            .map(Tool::exposure)
            .unwrap_or_default()
    }

    fn family(&self) -> Option<&str> {
        self.resolved_tool().and_then(Tool::family)
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        self.execute_with_context(
            args,
            ToolCallOptions {
                prefer_markdown: true,
            },
            None,
        )
        .await
    }

    async fn execute_with_context(
        &self,
        args: serde_json::Value,
        options: ToolCallOptions,
        context: Option<&dyn ToolRunContext>,
    ) -> anyhow::Result<ToolResult> {
        let Some(tool) = self.resolved_tool() else {
            tracing::warn!(tool = %self.name, "[tinyagents] shared tool not found");
            return Ok(ToolResult::error(format!("unknown tool '{}'", self.name)));
        };
        let result = tool.execute_with_context(args, options, context).await?;
        if !result.is_error {
            if let Some(hook) = &self.early_exit {
                hook.trigger(&self.name, result.output_for_llm(true));
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
#[path = "tools_canonical_tests.rs"]
mod tests;
