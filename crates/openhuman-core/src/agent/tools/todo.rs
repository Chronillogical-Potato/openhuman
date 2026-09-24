//! `todo` — the session's todo list, the way Claude Code and Codex have it.
//!
//! The tool itself is TinyAgents' `todos::TodoTool` (schema, argument
//! validation, the whole-list write, markdown). This file is only the host
//! adapter: it selects the thread-scoped list for a turn and registers the
//! harness dispatch. Bad arguments must become a tool error, never a fatal
//! harness error.
//!
//! **Scope key.** The list is keyed by the chat **thread id**
//! (`ToolRunContext::thread_id`) rather than `ParentExecutionContext::session_id`
//! — for the web channel, `session_id` is the `{client_id,thread_id}` JSON
//! blob (`fork_context.rs`), which changes with the client and is not what
//! `threads.todos_get` or the `thread_todos_changed` socket event key on. A
//! thread id is stable across reconnects and matches every other thread-scoped
//! surface (goals, turn state). Older lists written under the legacy
//! `session_id` key before this change are found via a one-time fallback read
//! (see [`current_scope`] / [`legacy_session_key`]) so an in-flight list isn't
//! dropped by the rekey.

use crate::agent::harness::fork_context::ParentExecutionContext;
use crate::agent::todos::ops::{self, TodoScope};
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use tinyagents_graph::todos as graph_todos;
use tinyagents_harness::context::RunContext;
use tinyagents_harness::tool::{ToolDispatch, ToolExecutionContext};
use tinytools::{PermissionLevel, Tool, ToolCallOptions, ToolResult, ToolRunContext};

pub struct TodoTool {
    inner: graph_todos::TodoTool,
    workspace_dir: PathBuf,
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
        let workspace_dir = match parent.data.parent.as_ref() {
            Some(parent_ctx) => parent_ctx.workspace_dir.clone(),
            None => crate::config::Config::load_or_init()
                .await
                .map(|c| c.workspace_dir)
                .map_err(|e| anyhow::anyhow!("[tool][todo] load config: {e}"))?,
        };
        TodoTool::new(workspace_dir)
            .execute_with_parent_context(arguments, parent.data.parent.clone(), Some(&context))
            .await
    }
}

impl TodoTool {
    pub fn new(workspace_dir: PathBuf) -> Self {
        Self {
            inner: graph_todos::TodoTool::new(ops::store(&workspace_dir)),
            workspace_dir,
        }
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
