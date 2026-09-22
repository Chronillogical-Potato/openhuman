//! `todo` — the session's todo list, the way Claude Code and Codex have it.
//!
//! The tool itself is TinyAgents' `todos::TodoTool` (schema, argument
//! validation, the whole-list write, markdown). This file is only the host
//! adapter: it decides **which** list a call is about — the agent session the
//! turn runs in, in memory for the life of the process — and registers the
//! harness dispatch. Nothing here may turn a bad argument into an `Err`: a
//! dispatch `Err` is fatal to the run, and a turn died that way when a model
//! sent the retired `{"cards": …}` shape to a previous host-side copy.

use crate::agent::harness::fork_context::ParentExecutionContext;
use crate::agent::todos::ops::{self, TodoScope};
use async_trait::async_trait;
use std::sync::Arc;
use tinyagents_graph::todos as graph_todos;
use tinyagents_harness::context::RunContext;
use tinyagents_harness::tool::{ToolDispatch, ToolExecutionContext};
use tinytools::{PermissionLevel, Tool, ToolCallOptions, ToolResult, ToolRunContext};

pub struct TodoTool {
    inner: graph_todos::TodoTool,
}

pub(crate) struct TodoToolDispatch {
    tool: Arc<dyn Tool>,
}
impl TodoToolDispatch {
    pub(crate) fn new(tool: Arc<dyn Tool>) -> Self {
        Self { tool }
    }
}
#[async_trait]
impl ToolDispatch<(), crate::agent::tinyagents::host::OpenHumanRunContext> for TodoToolDispatch {
    fn tool(&self) -> Arc<dyn Tool> {
        self.tool.clone()
    }
    async fn execute(
        &self,
        _state: &(),
        _call_id: tinyagents_harness::CallId,
        arguments: serde_json::Value,
        _options: ToolCallOptions,
        parent: &RunContext<crate::agent::tinyagents::host::OpenHumanRunContext>,
    ) -> anyhow::Result<ToolResult> {
        let context = ToolExecutionContext::from_run_context(parent, _call_id.clone());
        match TodoTool::new()
            .execute_with_parent_context(arguments, parent.data.parent.clone(), Some(&context))
            .await
        {
            Ok(result) => Ok(result),
            Err(error) => {
                tracing::warn!(%error, "[tool][todo] rejected call");
                Ok(ToolResult::error(format!("todo failed: {error}")))
            }
        }
    }
}

impl TodoTool {
    pub fn new() -> Self {
        Self {
            inner: graph_todos::TodoTool::new(ops::store()),
        }
    }
}

impl Default for TodoTool {
    fn default() -> Self {
        Self::new()
    }
}

/// The scope's store key, handed to the crate tool the only way it accepts
/// one: as the `thread_id` of a tool context. The crate keys a list by the
/// caller's thread; OpenHuman keys it by the agent session the turn runs in
/// (see [`current_scope`]), so the host substitutes its own key here rather
/// than letting the crate read a thread id that would address the wrong list.
struct ScopedKey<'a>(&'a str);

impl ToolRunContext for ScopedKey<'_> {
    fn thread_id(&self) -> Option<&str> {
        Some(self.0)
    }
}

#[async_trait]
impl Tool for TodoTool {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn parameters_schema(&self) -> serde_json::Value {
        self.inner.parameters_schema()
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::None
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        self.execute_with_context(args, ToolCallOptions::default(), None)
            .await
    }

    async fn execute_with_context(
        &self,
        args: serde_json::Value,
        _options: ToolCallOptions,
        tool_context: Option<&dyn ToolRunContext>,
    ) -> anyhow::Result<ToolResult> {
        self.execute_with_parent_context(args, None, tool_context)
            .await
    }
}

impl TodoTool {
    async fn execute_with_parent_context(
        &self,
        args: serde_json::Value,
        parent: Option<ParentExecutionContext>,
        tool_context: Option<&dyn ToolRunContext>,
    ) -> anyhow::Result<ToolResult> {
        let scope = current_scope(parent.as_ref(), tool_context);
        tracing::debug!(session_id = ?scope.session_id(), "[tool][todo] dispatch");
        let key = ScopedKey(scope.key());
        self.inner
            .execute_with_context(args, ToolCallOptions::default(), Some(&key))
            .await
    }
}

/// The list belongs to the agent session the tool runs in: the orchestrator's
/// session for a chat thread, a sub-agent's own session for its run. The
/// orchestrator used to be routed to one app-wide `orchestrator-tasks` board
/// instead; nothing rendered it, so the list the model kept was invisible to
/// the thread the user was looking at. The parent context names the session;
/// a tool that is only handed a thread id (older callers, tests) keys on that.
fn current_scope(
    parent: Option<&ParentExecutionContext>,
    tool_context: Option<&dyn ToolRunContext>,
) -> TodoScope {
    if let Some(parent) = parent {
        return TodoScope::Session {
            id: parent.session_id.clone(),
        };
    }
    match tool_context.and_then(ToolRunContext::thread_id) {
        Some(thread_id) => TodoScope::Session {
            id: thread_id.to_owned(),
        },
        None => TodoScope::Scratch,
    }
}

#[cfg(test)]
#[path = "todo_tests.rs"]
mod tests;
