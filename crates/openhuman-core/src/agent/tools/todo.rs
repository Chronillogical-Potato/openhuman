//! `todo` — the session's todo list, the way Claude Code and Codex have it.
//!
//! One call writes the whole list: `{"todos": [{"content", "status"}]}`.
//! There is no per-card CRUD, no approval gate, no evidence, no plan; the
//! list is a progress checklist the model rewrites as it works. It is scoped
//! to the agent session the turn runs in (in memory, for the life of the
//! process) via [`crate::agent::todos::ops`]; without a session (a bare
//! `execute` in a test) it falls back to a scratch list. Calling with no
//! `todos` returns the current list.

use crate::agent::harness::fork_context::ParentExecutionContext;
use crate::agent::todos::ops::{self, TodoScope};
use crate::agent::todos::types::{TaskBoardCard, TaskCardStatus};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tinyagents_harness::context::RunContext;
use tinyagents_harness::tool::{ToolDispatch, ToolExecutionContext};
use tinytools::{PermissionLevel, Tool, ToolCallOptions, ToolResult, ToolRunContext};

pub struct TodoTool;

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
        // A dispatch `Err` is fatal to the whole run in the harness
        // ("canonical execution errors remain fatal"), so nothing about a bad
        // argument may escape as one: it goes back to the model as a tool
        // error it can correct. A turn died this way when a model sent the
        // retired `{"cards": …}` shape.
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
        Self
    }
}

impl Default for TodoTool {
    fn default() -> Self {
        Self::new()
    }
}

/// One item as the model writes it. `status` accepts the Claude-style
/// `pending` / `in_progress` / `completed` plus the older `todo` / `done`
/// spellings the store already parses.
#[derive(Deserialize)]
struct TodoItem {
    content: String,
    #[serde(default)]
    status: Option<String>,
}

#[async_trait]
impl Tool for TodoTool {
    fn name(&self) -> &str {
        "todo"
    }

    fn description(&self) -> &str {
        "Your todo list for this conversation. Pass the complete list every time; it \
         replaces what was there. Use it for work with 3+ steps: write the steps up front, \
         keep exactly one `in_progress`, mark each `completed` the moment it is done. Omit \
         `todos` to read the current list."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "todos": {
                    "type": "array",
                    "description": "The full list, in order.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "content": { "type": "string" },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed"]
                            }
                        },
                        "required": ["content", "status"]
                    }
                }
            }
        })
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

        let result = match args.get("todos") {
            None | Some(serde_json::Value::Null) => match args.as_object() {
                // A write that used some other key (`cards`, `items`, …) is a
                // shape error to report, not a request to read the list.
                Some(map) if !map.is_empty() => Err(format!(
                    "unknown arguments {:?}: pass `todos` (the full list of \
                     {{content, status}}), or no arguments to read the list",
                    map.keys().collect::<Vec<_>>()
                )),
                _ => ops::list(&scope).await,
            },
            Some(raw) => match parse_items(raw) {
                Ok(cards) => ops::replace(&scope, cards).await,
                Err(error) => Err(error),
            },
        };

        match result {
            Ok(snap) => {
                let todos: Vec<serde_json::Value> = snap
                    .cards
                    .iter()
                    .map(|card| {
                        json!({
                            "content": card.title,
                            "status": wire_status(card.status),
                        })
                    })
                    .collect();
                let payload = json!({
                    "sessionId": snap.session_id,
                    "todos": todos,
                    "markdown": snap.markdown,
                });
                Ok(ToolResult::success(payload.to_string()))
            }
            Err(err) => Ok(ToolResult::error(err)),
        }
    }
}

/// Turn the model's `todos` array into store cards; every problem is a
/// message for the model, never a harness error.
fn parse_items(raw: &serde_json::Value) -> Result<Vec<TaskBoardCard>, String> {
    let items: Vec<TodoItem> =
        serde_json::from_value(raw.clone()).map_err(|e| format!("invalid `todos`: {e}"))?;
    let mut cards = Vec::with_capacity(items.len());
    for item in items {
        let content = item.content.trim();
        if content.is_empty() {
            return Err("every todo needs non-empty `content`".to_string());
        }
        let mut card = TaskBoardCard::new(content);
        card.status = match item.status.as_deref() {
            None => TaskCardStatus::Todo,
            Some(raw) => ops::parse_status(raw)?,
        };
        cards.push(card);
    }
    Ok(cards)
}

/// The three states the model is told about. Store states the list can no
/// longer produce (`ready`, `awaiting_approval`, `rejected`, `blocked`) fold
/// into the nearest one so an old thread still reads sensibly.
fn wire_status(status: TaskCardStatus) -> &'static str {
    match status {
        TaskCardStatus::InProgress => "in_progress",
        TaskCardStatus::Done | TaskCardStatus::Rejected => "completed",
        TaskCardStatus::Todo
        | TaskCardStatus::Ready
        | TaskCardStatus::AwaitingApproval
        | TaskCardStatus::Blocked => "pending",
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
