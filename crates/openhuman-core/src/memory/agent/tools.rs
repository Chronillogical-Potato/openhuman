//! Tool: `call_memory_agent` — invoke the memory retrieval agent to walk
//! the memory tree and return context for a query.
//!
//! Unlike the lower-level `memory_tree` tool (whose `walk`/`smart_walk` modes
//! now run the deterministic E2GraphRAG retriever and the other modes are
//! individual retrieval primitives), this tool spawns the full `agent_memory`
//! sub-agent which decides which retrieval strategies to combine and returns a
//! synthesised, cited answer.
//!
//! Supports both sync (blocking) and async (fire-and-forget) modes.

use crate::agent::harness::definition::AgentDefinitionRegistry;
use crate::agent::subagent_host::{
    run_subagent_with_parent, SubagentRunOptions, SubagentRunStatus,
};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use tinyagents_harness::context::{RunConfig, RunContext};
use tinyagents_harness::tool::{ToolDispatch, ToolExecutionContext};
use tinytools::ToolRunContext;
use tinytools::{PermissionLevel, Tool, ToolCallOptions, ToolCategory, ToolResult, ToolScope};

const AGENT_ID: &str = "agent_memory";

pub struct CallMemoryAgentTool;

/// Typed-harness dispatch for the nested memory agent. This is a real child
/// of the tool's live run, never a root reconstructed from a task-local.
pub(crate) struct CallMemoryAgentDispatch {
    tool: Arc<dyn Tool>,
}

impl CallMemoryAgentDispatch {
    pub(crate) fn new(tool: Arc<dyn Tool>) -> Self {
        Self { tool }
    }
}

#[async_trait]
impl ToolDispatch<(), crate::agent::tinyagents::host::OpenHumanRunContext>
    for CallMemoryAgentDispatch
{
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
        CallMemoryAgentTool::new()
            .execute_with_live_parent_context(arguments, Some(&context), Some(parent))
            .await
    }
}

impl CallMemoryAgentTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CallMemoryAgentTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CallMemoryAgentTool {
    fn name(&self) -> &str {
        "call_memory_agent"
    }

    fn description(&self) -> &str {
        "Invoke the memory retrieval agent to walk the memory tree and \
         return relevant context for a query. The agent autonomously \
         combines vector search, keyword matching, entity lookup, and \
         tree browsing — then returns a cited answer. Use this when you \
         need a comprehensive memory search rather than a single-strategy \
         lookup."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "required": ["query"],
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Natural-language query to search the user's memory for."
                },
                "context": {
                    "type": "string",
                    "description": "Optional context to help the memory agent understand what you're looking for and why."
                },
                "max_turns": {
                    "type": "integer",
                    "description": "Max retrieval turns the memory agent can take. Default: 15, hard cap: 20.",
                    "minimum": 1,
                    "maximum": 20
                },
                "async": {
                    "type": "boolean",
                    "description": "If true, fire-and-forget — the agent runs in the background and results are not returned inline. Default: false (synchronous)."
                }
            }
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn scope(&self) -> ToolScope {
        ToolScope::AgentOnly
    }

    fn is_concurrency_safe(&self, _args: &serde_json::Value) -> bool {
        true
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
        self.execute_with_live_parent_context(args, tool_context, None)
            .await
    }
}

impl CallMemoryAgentTool {
    /// Executes through the live TinyAgents parent. A direct `Tool` execution
    /// cannot synthesize a safe parent lineage, so it fails closed instead of
    /// silently creating a root subagent.
    async fn execute_with_live_parent_context(
        &self,
        args: serde_json::Value,
        tool_context: Option<&dyn ToolRunContext>,
        live_parent: Option<&RunContext<crate::agent::tinyagents::host::OpenHumanRunContext>>,
    ) -> anyhow::Result<ToolResult> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("call_memory_agent: `query` is required"))?;

        let context = args.get("context").and_then(|v| v.as_str());

        let max_turns = args
            .get("max_turns")
            .and_then(|v| v.as_u64())
            .map(|v| v.clamp(1, 20) as usize);

        let is_async = args.get("async").and_then(|v| v.as_bool()).unwrap_or(false);

        let Some(live_parent) = live_parent else {
            return Ok(ToolResult::error(
                "call_memory_agent: no live parent run — this tool must be \
                 called through the agent harness."
                    .to_string(),
            ));
        };

        let registry = AgentDefinitionRegistry::global()
            .ok_or_else(|| anyhow::anyhow!("call_memory_agent: agent registry not initialised"))?;

        let definition = registry
            .list()
            .iter()
            .find(|d| d.id == AGENT_ID)
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "call_memory_agent: agent definition '{AGENT_ID}' not found in registry"
                )
            })?;

        let parent = live_parent.data.parent.as_ref().ok_or_else(|| {
            anyhow::anyhow!("call_memory_agent: live run omitted parent agent context")
        })?;
        if !parent.allowed_subagent_ids.contains(AGENT_ID) {
            log::warn!(
                "[call_memory_agent] blocked memory subagent outside parent allowlist parent_agent={} requested_agent={} allowed={:?}",
                parent.agent_definition_id,
                AGENT_ID,
                parent.allowed_subagent_ids
            );
            return Ok(ToolResult::error(format!(
                "call_memory_agent: agent '{AGENT_ID}' is not in parent agent '{}' subagents.allowlist",
                parent.agent_definition_id
            )));
        }

        let mut prompt = format!(
            "Search the user's memory tree and return relevant context for this query:\n\n{query}"
        );
        if let Some(turns) = max_turns {
            prompt.push_str(&format!(
                "\n\nConstraint: use at most {turns} retrieval turns."
            ));
        }
        if let Some(ctx) = context {
            prompt.push_str(&format!("\n\nAdditional context:\n{ctx}"));
        }

        let task_id = format!("mem-{}", uuid::Uuid::new_v4());

        log::debug!(
            "[call_memory_agent] query_len={} async={} task_id={}",
            query.len(),
            is_async,
            task_id
        );

        let workspace_descriptor = tool_context.and_then(|ctx| ctx.workspace().cloned());
        let worktree_action_dir = workspace_descriptor
            .as_ref()
            .map(|descriptor| descriptor.root.clone());
        if let Some(descriptor) = workspace_descriptor.as_ref() {
            log::debug!(
                "[call_memory_agent] using ToolExecutionContext workspace root task_id={} workspace_root={} policy_id={}",
                task_id,
                descriptor.root.display(),
                descriptor.policy_id
            );
        }

        let options = SubagentRunOptions {
            task_id: Some(task_id.clone()),
            worktree_action_dir,
            workspace_descriptor,
            run_context: live_parent.data.child(),
            ..Default::default()
        };

        if is_async {
            let def = definition.clone();
            let prompt_clone = prompt.clone();
            let tid = task_id.clone();
            let detached_data = live_parent.data.detached_child();
            let detached_cancellation = detached_data.cancellation.clone();
            let detached_parent = live_parent
                .child(
                    RunConfig::new(format!("memory-agent-{task_id}")),
                    detached_data,
                )
                .map_err(|error| anyhow::anyhow!(error.to_string()))?
                .with_cancellation(detached_cancellation);
            tokio::spawn(async move {
                match run_subagent_with_parent(&detached_parent, def, prompt_clone, options).await {
                    Ok(outcome) => {
                        log::info!(
                            "[call_memory_agent] async task_id={} completed iterations={} elapsed={:?}",
                            tid,
                            outcome.iterations,
                            outcome.elapsed
                        );
                    }
                    Err(e) => {
                        log::warn!("[call_memory_agent] async task_id={} failed: {e:#}", tid);
                    }
                }
            });

            return Ok(ToolResult::success(format!(
                "Memory agent dispatched asynchronously (task_id: {task_id}). \
                 Results will be available when the agent completes."
            )));
        }

        // Synchronous path — block until the memory agent finishes.
        let started = std::time::Instant::now();
        match run_subagent_with_parent(live_parent, definition.to_owned(), prompt, options).await {
            Ok(outcome) => {
                let elapsed = started.elapsed();
                log::info!(
                    "[call_memory_agent] task_id={} completed iterations={} elapsed={:?} status={:?}",
                    task_id,
                    outcome.iterations,
                    elapsed,
                    outcome.status
                );

                let mut result = outcome.output;

                match outcome.status {
                    SubagentRunStatus::Completed => {}
                    SubagentRunStatus::AwaitingUser { question, .. } => {
                        result.push_str(&format!(
                            "\n\n⚠️ The memory agent needs clarification: {question}"
                        ));
                    }
                    SubagentRunStatus::Incomplete { reason } => {
                        result.push_str(&format!(
                            "\n\n⚠️ The memory agent stopped before finishing ({reason})."
                        ));
                    }
                    SubagentRunStatus::Cancelled => {
                        result.push_str("\n\n⚠️ The memory agent was cancelled.");
                    }
                }

                result.push_str(&format!(
                    "\n\n---\n_Memory agent: {} iterations, {:.1}s_",
                    outcome.iterations,
                    elapsed.as_secs_f64()
                ));

                Ok(ToolResult::success(result))
            }
            Err(e) => {
                log::warn!("[call_memory_agent] task_id={} failed: {e:#}", task_id);
                Ok(ToolResult::error(format!("Memory agent failed: {e}")))
            }
        }
    }
}
