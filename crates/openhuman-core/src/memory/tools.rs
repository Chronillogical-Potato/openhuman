//! The single `memory` agent tool: `recall | fetch | learn | forget`.
//!
//! Registered only while memory is on (see [`crate::tools::ops`]). `learn`
//! stamps the stored learning with what the host knows and the model does
//! not get to choose:
//!
//! - `workspace` — the agent's working folder (`action_dir`, or the turn's
//!   isolated workspace when one is scoped);
//! - `thread_id` and `agent_id`;
//! - `namespace` — the acting agent's memory node ([`super::scope`]), or,
//!   for `learn` with `share: true`, the nearest shared node above it (its
//!   team's, else the root);
//! - `tool_call` — this call's name and provider-assigned id;
//! - `source.kind = agent`.
//!
//! `recall`, `fetch` and `forget` are confined to the agent's reach: its own
//! node and the nodes it inherits, never a sibling agent's. A `reach` in the
//! model's filter is overwritten.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use async_trait::async_trait;
use serde_json::{json, Value};
use tinymemory::{MemoryMeta, Namespace, Reach, SourceKind, SourceRef, ToolCallRef};
use tinytools::{PermissionLevel, Tool, ToolCallOptions, ToolResult, ToolRunContext};

use crate::config::Config;
use crate::core::bus::BUS;
use crate::core::events::DomainEvent;

use super::error::MemoryError;
use super::ops;
use super::types::{FetchParams, ForgetParams, LearnParams, RecallParams, TurnCitation};

/// Most citations kept per thread between two drains.
const MAX_TURN_CITATIONS: usize = 20;

/// Citations `recall` produced during a thread's in-flight turn, drained by
/// the chat surface once the turn returns ([`take_turn_citations`]).
static TURN_CITATIONS: LazyLock<Mutex<HashMap<String, Vec<TurnCitation>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn record_turn_citations(thread_id: &str, citations: &[tinymemory::Citation]) {
    if citations.is_empty() {
        return;
    }
    let mut all = TURN_CITATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let entry = all.entry(thread_id.to_string()).or_default();
    for citation in citations {
        if entry.len() >= MAX_TURN_CITATIONS {
            break;
        }
        if !entry.iter().any(|existing| existing.id == citation.id.0) {
            entry.push(TurnCitation::from(citation));
        }
    }
}

/// Drains the citations `recall` produced for `thread_id` since the last
/// drain.
#[must_use]
pub fn take_turn_citations(thread_id: &str) -> Vec<TurnCitation> {
    TURN_CITATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(thread_id)
        .unwrap_or_default()
}

/// The tool's name.
pub const MEMORY_TOOL_NAME: &str = "memory";

/// The `memory` tool.
pub struct MemoryTool {
    config: Arc<Config>,
}

impl MemoryTool {
    /// A tool bound to `config`'s engine.
    #[must_use]
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

/// What the host knows about the call that the model does not set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallFacts {
    /// The agent's working folder.
    pub workspace: Option<String>,
    /// The thread the call runs in.
    pub thread_id: Option<String>,
    /// The agent that made the call.
    pub agent_id: Option<String>,
    /// The provider-assigned tool-call id.
    pub tool_call_id: Option<String>,
    /// The calling agent's memory node.
    pub namespace: Namespace,
    /// Where the agent's shared learnings go.
    pub shared_namespace: Namespace,
    /// What the agent may read.
    pub reach: Reach,
}

impl CallFacts {
    /// Gathers the facts from the run context and the turn's task-locals,
    /// falling back to `config.action_dir` for the workspace.
    #[must_use]
    pub fn gather(config: &Config, context: Option<&dyn ToolRunContext>) -> Self {
        let workspace = context
            .and_then(ToolRunContext::workspace_root)
            .map(std::path::Path::to_path_buf)
            .or_else(crate::agent::turn_workspace::current)
            .unwrap_or_else(|| config.action_dir.clone());
        let identity = super::scope::current().unwrap_or_else(|| {
            // Outside a scoped turn (a legacy driver), fall back to the
            // parent context's agent, else the root.
            crate::agent::harness::fork_context::current_parent()
                .map_or_else(super::scope::MemoryIdentity::root, |parent| {
                    super::scope::MemoryIdentity::agent(&parent.agent_definition_id)
                })
        });
        Self {
            workspace: Some(workspace.display().to_string()),
            thread_id: context
                .and_then(ToolRunContext::thread_id)
                .map(str::to_string),
            tool_call_id: crate::tools::host_extensions::tool_call_id(context),
            ..Self::of(config, &identity)
        }
    }

    /// The facts of a call made by `identity` under `config`, with no run
    /// context.
    #[must_use]
    pub fn of(config: &Config, identity: &super::scope::MemoryIdentity) -> Self {
        let namespace = identity.namespace(config);
        Self {
            agent_id: identity.agent_id.clone(),
            shared_namespace: namespace.shared_ancestor(),
            reach: identity.reach(config),
            namespace,
            ..Self::default()
        }
    }

    /// The metadata `learn` stamps on the stored item: at the agent's own
    /// node, or at its shared node when `share`.
    #[must_use]
    pub fn learn_meta(&self, share: bool) -> MemoryMeta {
        MemoryMeta {
            namespace: if share {
                self.shared_namespace.clone()
            } else {
                self.namespace.clone()
            },
            workspace: self.workspace.clone(),
            thread_id: self.thread_id.clone(),
            agent_id: self.agent_id.clone(),
            tool_call: Some(ToolCallRef {
                name: MEMORY_TOOL_NAME.to_string(),
                id: self.tool_call_id.clone(),
            }),
            source: SourceRef {
                kind: SourceKind::Agent,
                id: None,
            },
            ..MemoryMeta::default()
        }
    }
}

fn arg_str(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn parse<T: serde::de::DeserializeOwned>(args: &Value) -> Result<T, String> {
    serde_json::from_value(args.clone()).map_err(|error| format!("invalid arguments: {error}"))
}

/// Runs one action. Errors become a model-visible tool error, never a hard
/// failure, so the model can adapt (memory off, unsupported mode, …).
pub async fn run_action(config: &Config, args: &Value, facts: &CallFacts) -> ToolResult {
    let action = arg_str(args, "action").unwrap_or_default();
    tracing::debug!(action = %action, "[memory:tool] dispatch");
    let result: Result<Value, String> = match action.as_str() {
        "recall" => match parse::<RecallParams>(args) {
            Ok(mut params) => {
                confine(&mut params.filter, facts);
                let question = params.question.clone();
                ops::recall(config, params)
                    .await
                    .map(|view| {
                        if let Some(thread_id) = facts.thread_id.as_deref() {
                            record_turn_citations(thread_id, &view.citations);
                        }
                        BUS.publish(DomainEvent::MemoryRecalled {
                            query: question,
                            hit_count: view.citations.len(),
                        });
                        json!(view)
                    })
                    .map_err(render_error)
            }
            Err(error) => Err(error),
        },
        "fetch" => match parse::<FetchParams>(args) {
            Ok(mut params) => {
                confine(&mut params.filter, facts);
                ops::fetch(config, params)
                    .await
                    .map(|view| json!(view))
                    .map_err(render_error)
            }
            Err(error) => Err(error),
        },
        "learn" => match parse::<LearnParams>(args) {
            Ok(mut params) => {
                params.meta = None;
                let kind = params.kind.unwrap_or(tinymemory::LearningKind::Fact);
                let share = args.get("share").and_then(Value::as_bool) == Some(true);
                ops::learn(config, params, Some(facts.learn_meta(share)))
                    .await
                    .map(|view| {
                        BUS.publish(DomainEvent::MemoryStored {
                            key: view.id.clone(),
                            category: kind.as_str().to_string(),
                            namespace: "learnings".to_string(),
                        });
                        json!(view)
                    })
                    .map_err(render_error)
            }
            Err(error) => Err(error),
        },
        "forget" => match parse::<ForgetParams>(args) {
            Ok(mut params) => {
                params.reach = Some(facts.reach.clone());
                ops::forget(config, params)
                    .await
                    .map(|view| json!(view))
                    .map_err(render_error)
            }
            Err(error) => Err(error),
        },
        other => Err(format!(
            "unknown action `{other}`; use one of recall, fetch, learn, forget"
        )),
    };
    match result {
        Ok(value) => ToolResult::success(value.to_string()),
        Err(message) => ToolResult::error(message),
    }
}

/// Confines a model-supplied filter to the calling agent's reach.
fn confine(filter: &mut Option<tinymemory::MetaFilter>, facts: &CallFacts) {
    filter.get_or_insert_with(Default::default).reach = Some(facts.reach.clone());
}

fn render_error(error: MemoryError) -> String {
    format!("{} ({})", error, error.code())
}

#[async_trait]
impl Tool for MemoryTool {
    fn name(&self) -> &str {
        MEMORY_TOOL_NAME
    }

    fn description(&self) -> &str {
        "Long-term memory across conversations, documents and learnings. \
         `recall` answers a question from memory with citations; `fetch` returns raw \
         matching items (filter by metadata such as workspace, repo, file_path, kinds); \
         `learn` stores a durable fact, preference, procedure or correction about the \
         user or their work — in your own memory, or with `share: true` in the memory \
         your team or every agent shares; `forget` removes items by id. You read your own \
         memory plus what is shared with you. Recall before asking the user something they \
         may already have told you; learn things worth remembering next time."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["recall", "fetch", "learn", "forget"],
                    "description": "What to do."
                },
                "question": {"type": "string", "description": "recall: the question to answer from memory."},
                "query": {"type": "string", "description": "fetch: what to search for."},
                "mode": {"type": "string", "enum": ["hybrid", "keyword", "vector"], "description": "fetch: retrieval mode; only the engine's declared modes work (hybrid is always safe)."},
                "filter": {"type": "object", "description": "recall/fetch: metadata filter, e.g. {\"kinds\": [\"learning\"], \"workspace\": \"/path\", \"repo\": \"owner/name\"}."},
                "limit": {"type": "integer", "minimum": 1, "maximum": 100, "description": "recall/fetch: most results."},
                "cursor": {"type": "string", "description": "fetch: next-page cursor."},
                "text": {"type": "string", "description": "learn: the learning, one self-contained sentence."},
                "kind": {"type": "string", "enum": ["preference", "fact", "procedure", "correction", "other"], "description": "learn: what kind of learning (default fact)."},
                "confidence": {"type": "number", "minimum": 0, "maximum": 1, "description": "learn: confidence (default 0.8)."},
                "share": {"type": "boolean", "description": "learn: store in shared memory every agent (or your team) reads, instead of your own (default false)."},
                "ids": {"type": "array", "items": {"type": "string"}, "description": "forget: item ids to remove."}
            },
            "required": ["action"]
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    fn permission_level_with_args(&self, args: &Value) -> PermissionLevel {
        match args.get("action").and_then(Value::as_str) {
            Some("recall" | "fetch") => PermissionLevel::ReadOnly,
            _ => PermissionLevel::Write,
        }
    }

    fn is_concurrency_safe(&self, args: &Value) -> bool {
        matches!(
            args.get("action").and_then(Value::as_str),
            Some("recall" | "fetch")
        )
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        self.execute_with_context(args, ToolCallOptions::default(), None)
            .await
    }

    async fn execute_with_context(
        &self,
        args: Value,
        _options: ToolCallOptions,
        context: Option<&dyn ToolRunContext>,
    ) -> anyhow::Result<ToolResult> {
        let facts = CallFacts::gather(&self.config, context);
        Ok(run_action(&self.config, &args, &facts).await)
    }
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
