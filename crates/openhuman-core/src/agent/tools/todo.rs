//! `todo` — the session's todo list, the way Claude Code and Codex have it.
//!
//! The tool itself is TinyAgents' `todos::TodoTool` (schema, argument
//! validation, the whole-list write, markdown). This file is only the host
//! adapter: it selects the session-scoped list for a turn and registers the
//! harness dispatch. Bad arguments must become a tool error, never a fatal
//! harness error.

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
        call_id: tinyagents_harness::CallId,
        arguments: serde_json::Value,
        _options: ToolCallOptions,
        parent: &RunContext<crate::agent::tinyagents::host::OpenHumanRunContext>,
    ) -> anyhow::Result<ToolResult> {
        let context = ToolExecutionContext::from_run_context(parent, call_id);
        TodoTool::new()
            .execute_with_parent_context(arguments, parent.data.parent.clone(), Some(&context))
            .await
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

/// Supplies the selected session key to TinyAgents' tool implementation.
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
