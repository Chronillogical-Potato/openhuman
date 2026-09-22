//! `todo` — unified CRUD tool for the agent's task board.
//!
//! Dispatches on the `op` field so a single tool exposes
//! `add` / `edit` / `update_status` / `remove` / `replace` / `clear` /
//! `list`. The board is persisted to the active thread (when there is
//! one) via [`crate::agent::todos::ops`]; without a caller thread the
//! tool falls back to a process-global scratch list. Returns a markdown
//! rendering so transcripts read cleanly.

use crate::agent::harness::fork_context::ParentExecutionContext;
use crate::agent::todos::ops::{self, BoardLocation, CardPatch};
use crate::agent::todos::types::{TaskApprovalMode, TaskBoardCard, TaskCardStatus};
use async_trait::async_trait;
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
        TodoTool::new()
            .execute_with_parent_context(arguments, parent.data.parent.clone(), Some(&context))
            .await
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

#[async_trait]
impl Tool for TodoTool {
    fn name(&self) -> &str {
        "todo"
    }

    fn description(&self) -> &str {
        "The thread's visible task list; cards persist across turns. Use for requests with \
         3+ steps. Keep one `in_progress`; mark cards `done` as soon as they are, and \
         `blocked` with a `blocker`. The board binds automatically; do not pass a thread id."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        // The parser still accepts `objective`, `plan`, `allowedTools`,
        // `approvalMode` and `acceptanceCriteria` (dispatched boards set them
        // through the task RPCs), but they are not advertised: a chat agent
        // never filled them and each cost every turn a slice of schema.
        json!({
            "type": "object",
            "properties": {
                "op": {
                    "type": "string",
                    "enum": ["add", "edit", "update_status", "decide_plan", "remove", "replace", "clear", "list"]
                },
                "id": { "type": "string", "description": "Card id (edit/update_status/remove)." },
                "content": { "type": "string", "description": "Card title (add; optional for edit)." },
                "status": {
                    "type": "string",
                    "enum": ["todo", "pending", "in_progress", "blocked", "done", "completed"]
                },
                "notes": { "type": "string" },
                "blocker": { "type": "string" },
                "approve": {
                    "type": "boolean",
                    "description": "decide_plan: approve (true) or reject (false) a card awaiting approval."
                },
                "evidence": {
                    "type": "array",
                    "description": "Verification output, links or files produced for the card.",
                    "items": { "type": "string" }
                },
                "cards": {
                    "type": "array",
                    "description": "Full card list for op=replace.",
                    "items": { "type": "object" }
                }
            },
            "required": ["op"]
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
        let op = args
            .get("op")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("missing required field `op`"))?
            .trim()
            .to_string();

        let location = current_location(parent.as_ref(), tool_context);
        tracing::debug!(op = %op, thread_id = ?location.thread_id(), "[tool][todo] dispatch");

        let result = match op.as_str() {
            "add" => {
                let content = required_string(&args, "content")?;
                let mut patch = patch_from_args(&args)?;
                if patch.approval_mode.is_none() {
                    patch.approval_mode = Some(default_task_approval_mode().await);
                }
                ops::add(&location, &content, patch).await
            }
            "edit" => {
                let id = required_string(&args, "id")?;
                let mut patch = patch_from_args(&args)?;
                patch.content = optional_string(&args, "content");
                ops::edit(&location, &id, patch).await
            }
            "update_status" => {
                let id = required_string(&args, "id")?;
                let status = required_string(&args, "status")?;
                let status = ops::parse_status(&status).map_err(anyhow::Error::msg)?;
                ops::update_status(&location, &id, status).await
            }
            "remove" => {
                let id = required_string(&args, "id")?;
                ops::remove(&location, &id).await
            }
            "replace" => {
                let cards = args
                    .get("cards")
                    .ok_or_else(|| anyhow::anyhow!("missing `cards` for op=replace"))?;
                let cards: Vec<TaskBoardCard> = serde_json::from_value(cards.clone())
                    .map_err(|e| anyhow::anyhow!("invalid `cards`: {e}"))?;
                ops::replace(&location, cards).await
            }
            "decide_plan" => {
                let id = required_string(&args, "id")?;
                let approve = args
                    .get("approve")
                    .and_then(serde_json::Value::as_bool)
                    .ok_or_else(|| anyhow::anyhow!("missing required boolean `approve`"))?;
                ops::decide_plan(&location, &id, approve).await
            }
            "clear" => ops::clear(&location).await,
            "list" => ops::list(&location).await,
            other => {
                return Ok(ToolResult::error(format!(
                    "unknown op '{other}' (expected \
                 add|edit|update_status|decide_plan|remove|replace|clear|list)"
                )));
            }
        };

        match result {
            Ok(snap) => {
                let payload = json!({
                    "threadId": snap.thread_id,
                    "cards": snap.cards,
                    "markdown": snap.markdown,
                });
                Ok(ToolResult::success(payload.to_string()))
            }
            Err(err) => Ok(ToolResult::error(err)),
        }
    }
}

async fn default_task_approval_mode() -> Option<TaskApprovalMode> {
    // Interactive plan review is handled by the `request_plan_review` gate
    // (it parks the live turn), NOT by stamping conversation-thread cards: the
    // background dispatcher never sweeps conversation boards, so a card status
    // can't gate a chat turn. This default therefore just carries the
    // config-driven behaviour for the dispatched boards (`user-tasks` /
    // `task-sources`).
    match crate::config::ops::load_config_with_timeout().await {
        Ok(config) => Some(if config.autonomy.require_task_plan_approval {
            TaskApprovalMode::Required
        } else {
            TaskApprovalMode::NotRequired
        }),
        Err(err) => {
            tracing::debug!(
                error = %err,
                "[tool][todo] failed to load config for task approval default"
            );
            None
        }
    }
}

fn current_location(
    parent: Option<&ParentExecutionContext>,
    tool_context: Option<&dyn ToolRunContext>,
) -> BoardLocation {
    let Some(parent) = parent else {
        return BoardLocation::Scratch;
    };
    // Every agent, the orchestrator included, binds to the conversation thread
    // it is running in. The orchestrator used to be routed to one app-wide
    // `orchestrator-tasks` board instead; that board is deprecated and nothing
    // renders it, so cards written there were invisible to the thread the
    // user was looking at.
    let Some(thread_id) = tool_context.and_then(ToolRunContext::thread_id) else {
        return BoardLocation::Scratch;
    };
    BoardLocation::Thread {
        workspace_dir: parent.workspace_dir.clone(),
        thread_id: thread_id.to_owned(),
    }
}

fn required_string(args: &serde_json::Value, key: &str) -> anyhow::Result<String> {
    let value = args
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing required field `{key}`"))?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(anyhow::anyhow!("missing required field `{key}`"));
    }
    Ok(trimmed.to_string())
}

fn optional_string(args: &serde_json::Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn patch_from_args(args: &serde_json::Value) -> anyhow::Result<CardPatch> {
    let status: Option<TaskCardStatus> = match args.get("status").and_then(|v| v.as_str()) {
        Some(s) => Some(ops::parse_status(s).map_err(anyhow::Error::msg)?),
        None => None,
    };
    let approval_mode = match args.get("approvalMode") {
        Some(value) if value.is_null() => Some(None),
        Some(value) => match value.as_str() {
            Some("required") => Some(Some(TaskApprovalMode::Required)),
            Some("not_required") => Some(Some(TaskApprovalMode::NotRequired)),
            Some(other) => {
                return Err(anyhow::anyhow!(
                    "invalid approvalMode '{other}' (expected required|not_required|null)"
                ));
            }
            None => {
                return Err(anyhow::anyhow!(
                    "invalid approvalMode type (expected required|not_required|null)"
                ));
            }
        },
        None => None,
    };
    Ok(CardPatch {
        content: None,
        status,
        objective: optional_string(args, "objective"),
        plan: optional_string_array(args, "plan")?,
        allowed_tools: optional_string_array(args, "allowedTools")?,
        approval_mode,
        acceptance_criteria: optional_string_array(args, "acceptanceCriteria")?,
        evidence: optional_string_array(args, "evidence")?,
        notes: optional_string(args, "notes"),
        blocker: optional_string(args, "blocker"),
        source_metadata: None,
    })
}

fn optional_string_array(
    args: &serde_json::Value,
    key: &str,
) -> anyhow::Result<Option<Vec<String>>> {
    let Some(value) = args.get(key) else {
        return Ok(None);
    };
    let values = value
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("`{key}` must be an array of strings"))?;
    values
        .iter()
        .map(|item| {
            item.as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| anyhow::anyhow!("`{key}` must be an array of strings"))
        })
        .collect::<anyhow::Result<Vec<_>>>()
        .map(Some)
}

#[cfg(test)]
#[path = "todo_tests.rs"]
mod tests;
