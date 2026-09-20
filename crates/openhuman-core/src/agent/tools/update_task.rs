//! `update_task` — move or update a specific task card on a thread's board.
//!
//! `todo` only reaches the *current* thread's board. `update_task` addresses a
//! card **by id** on a *target* board — defaulting to the proactive
//! `task-sources` board — so the orchestrator can advance the task it is
//! working: move it to
//! `in_progress`/`blocked`/`done`, or update its objective/notes/evidence/blocker.
//!
//! It is a thin wrapper over [`crate::agent::todos::ops::edit`], which
//! applies the status move + field updates atomically and enforces the
//! single-`in_progress` invariant.

use crate::agent::harness::fork_context::ParentExecutionContext;
use crate::agent::todos::ops::{self, BoardLocation, CardPatch};
use crate::integrations::task_sources::TASK_SOURCES_THREAD_ID;
use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tinyagents_harness::context::RunContext;
use tinyagents_harness::tool::ToolDispatch;
use tinytools::{PermissionLevel, Tool, ToolCallOptions, ToolResult};

pub struct UpdateTaskTool;

pub(crate) struct UpdateTaskDispatch {
    tool: Arc<dyn Tool>,
}
impl UpdateTaskDispatch {
    pub(crate) fn new(tool: Arc<dyn Tool>) -> Self {
        Self { tool }
    }
}
#[async_trait]
impl ToolDispatch<(), crate::agent::tinyagents::host::OpenHumanRunContext> for UpdateTaskDispatch {
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
        UpdateTaskTool::new()
            .execute_with_parent_context(arguments, parent.data.parent.clone())
            .await
    }
}

impl UpdateTaskTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UpdateTaskTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for UpdateTaskTool {
    fn name(&self) -> &str {
        "update_task"
    }

    fn description(&self) -> &str {
        "Update one task card by `id`: move it between columns via `status`, and/or revise its other fields. Finish with `status: done` plus `evidence`; if you cannot proceed, `status: blocked` plus a `blocker`. At most one card may be `in_progress`. Defaults to the proactive `task-sources` board."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Id of the card to move/update (required)." },
                "status": {
                    "type": "string",
                    "enum": ["todo", "in_progress", "blocked", "done"],
                    "description": "New status — moves the card to that column."
                },
                "objective": { "type": "string", "description": "Updated desired outcome for the task." },
                "notes": { "type": "string", "description": "Progress notes / running summary." },
                "blocker": { "type": "string", "description": "Why the task is blocked (set with status=blocked)." },
                "evidence": {
                    "type": "array",
                    "description": "Links, output, or files proving the work (set with status=done).",
                    "items": { "type": "string" }
                },
                "plan": {
                    "type": "array",
                    "description": "Updated ordered execution steps.",
                    "items": { "type": "string" }
                },
                "acceptanceCriteria": {
                    "type": "array",
                    "description": "Updated checklist that must hold before the task is done.",
                    "items": { "type": "string" }
                },
                "threadId": {
                    "type": "string",
                    "description": "Board to target; defaults to the `task-sources` board."
                }
            },
            "required": ["id"]
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::None
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        self.execute_with_parent_context(args, None).await
    }
}

impl UpdateTaskTool {
    async fn execute_with_parent_context(
        &self,
        args: serde_json::Value,
        parent: Option<ParentExecutionContext>,
    ) -> anyhow::Result<ToolResult> {
        let Some(id) = optional_string(&args, "id") else {
            return Ok(ToolResult::error("missing required field `id`".to_string()));
        };

        let patch = match build_patch(&args) {
            Ok(patch) => patch,
            Err(err) => return Ok(ToolResult::error(err)),
        };
        if patch_is_empty(&patch) {
            return Ok(ToolResult::error(
                "nothing to update — provide `status` and/or a field \
                 (objective/notes/evidence/blocker/plan/acceptanceCriteria)"
                    .to_string(),
            ));
        }

        let location = match resolve_location(&args, parent.as_ref()).await {
            Ok(location) => location,
            Err(err) => return Ok(ToolResult::error(err)),
        };

        Ok(apply(&location, &id, patch).await)
    }
}

/// Apply the move/update to the card and render the result. Split out from
/// `execute` so the edit + response shaping is testable without a fork/thread
/// context (which `resolve_location` needs).
async fn apply(location: &BoardLocation, id: &str, patch: CardPatch) -> ToolResult {
    tracing::info!(
        card_id = %id,
        thread_id = ?location.thread_id(),
        status = ?patch.status,
        "[tool][update_task] move/update task card"
    );
    match ops::edit(location, id, patch).await {
        Ok(snap) => {
            let payload = json!({
                "threadId": snap.thread_id,
                "cards": snap.cards,
                "markdown": snap.markdown,
            });
            ToolResult::success(payload.to_string())
        }
        Err(err) => ToolResult::error(err),
    }
}

/// Resolve the board to act on: the explicit `threadId` arg, else the proactive
/// `task-sources` board. The workspace root comes from the explicit parent
/// carrier when present, otherwise from the loaded config.
async fn resolve_location(
    args: &serde_json::Value,
    parent: Option<&ParentExecutionContext>,
) -> Result<BoardLocation, String> {
    let thread_id =
        optional_string(args, "threadId").unwrap_or_else(|| TASK_SOURCES_THREAD_ID.to_string());
    Ok(BoardLocation::Thread {
        workspace_dir: workspace_dir(parent).await?,
        thread_id,
    })
}

async fn workspace_dir(parent: Option<&ParentExecutionContext>) -> Result<PathBuf, String> {
    if let Some(parent) = parent {
        return Ok(parent.workspace_dir.clone());
    }
    crate::config::ops::load_config_with_timeout()
        .await
        .map(|config| config.workspace_dir)
        .map_err(|e| format!("update_task: failed to load config for workspace dir: {e}"))
}

fn build_patch(args: &serde_json::Value) -> Result<CardPatch, String> {
    let status = match args.get("status").and_then(|v| v.as_str()) {
        Some(s) => Some(ops::parse_status(s)?),
        None => None,
    };
    Ok(CardPatch {
        status,
        objective: optional_string(args, "objective"),
        plan: optional_string_array(args, "plan")?,
        acceptance_criteria: optional_string_array(args, "acceptanceCriteria")?,
        evidence: optional_string_array(args, "evidence")?,
        notes: optional_string(args, "notes"),
        blocker: optional_string(args, "blocker"),
        ..Default::default()
    })
}

fn patch_is_empty(patch: &CardPatch) -> bool {
    patch.status.is_none()
        && patch.objective.is_none()
        && patch.plan.is_none()
        && patch.acceptance_criteria.is_none()
        && patch.evidence.is_none()
        && patch.notes.is_none()
        && patch.blocker.is_none()
}

fn optional_string(args: &serde_json::Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn optional_string_array(
    args: &serde_json::Value,
    key: &str,
) -> Result<Option<Vec<String>>, String> {
    match args.get(key) {
        None => Ok(None),
        Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Array(items)) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                let s = item
                    .as_str()
                    .ok_or_else(|| format!("`{key}` must be an array of strings"))?
                    .trim();
                if !s.is_empty() {
                    out.push(s.to_string());
                }
            }
            Ok(Some(out))
        }
        Some(_) => Err(format!("`{key}` must be an array of strings")),
    }
}

#[cfg(test)]
#[path = "update_task_tests.rs"]
mod tests;
