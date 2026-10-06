//! Tool-inventory coverage for [`assemble_turn_harness`]: which of the turn's
//! shared tools end up registered on the assembled harness.

use super::*;
use crate::agent::tinyagents::TurnModelSource;
use async_trait::async_trait;
use tinyinference_llm::model::{ChatModel, ModelProfile, ModelRequest, ModelResponse};
use tinytools::{Tool, ToolResult};

const GOAL_TOOLS: [&str; 3] = ["goal_get", "goal_set", "goal_complete"];

struct PlainTool;

#[async_trait]
impl Tool for PlainTool {
    fn name(&self) -> &str {
        "plain_tool"
    }

    fn description(&self) -> &str {
        "thread-independent test tool"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }

    async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<ToolResult> {
        Ok(ToolResult::success("ok"))
    }
}

struct IdleModel;

#[async_trait]
impl ChatModel<()> for IdleModel {
    fn profile(&self) -> Option<&ModelProfile> {
        static PROFILE: std::sync::OnceLock<ModelProfile> = std::sync::OnceLock::new();
        Some(PROFILE.get_or_init(|| {
            let mut profile = ModelProfile::default();
            profile.tool_calling = true;
            profile
        }))
    }

    async fn invoke(
        &self,
        _state: &(),
        _request: ModelRequest,
    ) -> tinyinference_llm::Result<ModelResponse> {
        Ok(ModelResponse::assistant("done"))
    }
}

/// Assemble a harness over the real goal tools plus one ordinary tool and
/// return the names it registered.
fn registered_tool_names(has_thread: bool) -> Vec<String> {
    let workspace = tempfile::TempDir::new().expect("workspace");
    let model: Arc<dyn ChatModel<()>> = Arc::new(IdleModel);
    let models = TurnModelSource::from_model(model)
        .build("assembly-test-model", 0.0, None, None)
        .expect("scripted turn models build");
    let mut tools = crate::agent::goals::goal_tools(workspace.path());
    tools.push(Box::new(PlainTool));

    let assembled = assemble_turn_harness(
        models,
        "assembly-test-model",
        vec![Arc::new(tools)],
        None,
        3,
        None,
        None,
        None,
        &[],
        TurnContextMiddleware::default(),
        Vec::new(),
        None,
        None,
        false,
        false,
        false,
        tinyagents_harness::config::ToolDispatcher::default(),
        Arc::new(HashSet::new()),
        None,
        None,
        has_thread,
        None,
    );
    assembled.harness.tools().names()
}

#[test]
fn goal_tools_are_not_registered_on_a_turn_without_a_thread() {
    let names = registered_tool_names(false);

    for goal_tool in GOAL_TOOLS {
        assert!(
            !names.iter().any(|name| name == goal_tool),
            "{goal_tool} must not be offered to a thread-less turn: {names:?}"
        );
    }
    assert!(
        names.iter().any(|name| name == "plain_tool"),
        "thread-independent tools still register: {names:?}"
    );
}

#[test]
fn goal_tools_are_registered_on_a_threaded_turn() {
    let names = registered_tool_names(true);

    for goal_tool in GOAL_TOOLS.iter().chain(["plain_tool"].iter()) {
        assert!(
            names.iter().any(|name| name == goal_tool),
            "{goal_tool} must be offered to a threaded turn: {names:?}"
        );
    }
}
