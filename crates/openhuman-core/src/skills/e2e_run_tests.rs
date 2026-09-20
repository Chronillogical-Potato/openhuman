//! Mocked-LLM e2e tests for generic workflow-run plumbing.
//!
//! These ignored, serial tests cover the two workflow behaviours independent
//! of the removed dispatcher/task-board surface: an inner workflow reaches a
//! terminal footer and an orchestrator consumes that result via `run_workflow`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use crate::agent::harness::run_channel_turn_via_graph;
use crate::agent::messages::ChatMessage;
use crate::agent::tools::RunWorkflowTool;
use crate::config::{MultimodalConfig, MultimodalFileConfig};
use crate::inference::provider::factory::test_provider_override;
use crate::skills::runtime::{await_run_outcome, spawn_workflow_run_background};
use tinyinference_llm::message::AssistantMessage;
use tinyinference_llm::model::{ChatModel, ModelProfile, ModelRequest, ModelResponse};
use tinyinference_llm::tool::ToolCall;
use tinytools::Tool;

fn serial() -> &'static tokio::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

struct WorkspaceEnv {
    previous: Option<String>,
}

impl WorkspaceEnv {
    fn set(path: &std::path::Path) -> Self {
        let previous = std::env::var("OPENHUMAN_WORKSPACE").ok();
        std::env::set_var("OPENHUMAN_WORKSPACE", path);
        Self { previous }
    }
}

impl Drop for WorkspaceEnv {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var("OPENHUMAN_WORKSPACE", value),
            None => std::env::remove_var("OPENHUMAN_WORKSPACE"),
        }
    }
}

struct MockLlm {
    workflow_id: Option<String>,
    workflow_call_emitted: AtomicBool,
}

impl MockLlm {
    fn new(workflow_id: Option<&str>) -> Arc<Self> {
        Arc::new(Self {
            workflow_id: workflow_id.map(str::to_owned),
            workflow_call_emitted: AtomicBool::new(false),
        })
    }
}

fn final_text(text: &str) -> ModelResponse {
    ModelResponse::assistant(text)
}

fn tool_call_response(id: &str, name: &str, arguments: serde_json::Value) -> ModelResponse {
    ModelResponse {
        message: AssistantMessage {
            id: None,
            content: Vec::new(),
            tool_calls: vec![ToolCall::new(id, name, arguments)],
            usage: None,
        },
        usage: None,
        finish_reason: Some("tool_calls".to_string()),
        raw: None,
        resolved_model: None,
        continue_turn: None,
        served_from_cache: false,
        correlation: None,
        resolved_route: None,
    }
}

#[async_trait]
impl ChatModel<()> for MockLlm {
    async fn invoke(
        &self,
        _state: &(),
        request: ModelRequest,
    ) -> tinyinference_llm::Result<ModelResponse> {
        let conversation = request
            .messages
            .iter()
            .map(|message| message.text())
            .collect::<Vec<_>>()
            .join("\n");
        if conversation.contains("running a single workflow")
            || conversation.contains("Workflow guidelines")
        {
            return Ok(final_text("WORKFLOW_DONE: inbox triaged"));
        }
        if conversation.contains("WORKFLOW_DONE") || conversation.contains("\"status\"") {
            return Ok(final_text("ORCHESTRATOR_DONE"));
        }
        match &self.workflow_id {
            Some(id) if !self.workflow_call_emitted.swap(true, Ordering::SeqCst) => {
                Ok(tool_call_response(
                    "c1",
                    "run_workflow",
                    serde_json::json!({ "workflow_id": id, "wait_seconds": 20 }),
                ))
            }
            Some(_) => Ok(final_text("WORKFLOW_DONE: inbox triaged")),
            None => Ok(final_text("TASK_DONE_NO_WORKFLOW")),
        }
    }
}

fn seed_runnable_workflow(workspace: &std::path::Path, id: &str) {
    let directory = workspace.join("skills").join(id);
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("skill.toml"),
        format!("id = \"{id}\"\nwhen_to_use = \"triage email\"\n"),
    )
    .unwrap();
    std::fs::write(
        directory.join("SKILL.md"),
        format!(
            "---\nname: {id}\ndescription: Triage the inbox.\n---\n\nSummarise and label the inbox.\n"
        ),
    )
    .unwrap();
}

#[ignore = "process-global model override + OPENHUMAN_WORKSPACE; run serially"]
#[tokio::test]
async fn inner_workflow_run_executes_via_mock_llm_and_reaches_done() {
    let _serial = serial().lock().await;
    let workspace_root = tempfile::tempdir().unwrap();
    let _environment = WorkspaceEnv::set(workspace_root.path());
    let workspace = crate::skills::schemas::resolve_workspace_dir().await;
    seed_runnable_workflow(&workspace, "triage-inbox");
    let _provider = test_provider_override::install_model(MockLlm::new(Some("triage-inbox")));

    let started = spawn_workflow_run_background("triage-inbox".to_string(), None)
        .await
        .expect("runnable workflow should spawn");
    let outcome = await_run_outcome(&started.log_path, Duration::from_secs(20))
        .await
        .unwrap_or_else(|| panic!("inner workflow did not reach a terminal footer"));

    assert_eq!(outcome.status, "DONE");
    assert!(outcome.output.contains("WORKFLOW_DONE"));
}

#[ignore = "process-global model override + OPENHUMAN_WORKSPACE; run serially"]
#[tokio::test]
async fn orchestrator_runs_workflow_tool_and_gets_inner_result() {
    let _serial = serial().lock().await;
    let workspace_root = tempfile::tempdir().unwrap();
    let _environment = WorkspaceEnv::set(workspace_root.path());
    let workspace = crate::skills::schemas::resolve_workspace_dir().await;
    seed_runnable_workflow(&workspace, "triage-inbox");
    let mock = MockLlm::new(Some("triage-inbox"));
    let _provider = test_provider_override::install_model(mock.clone());
    let model: Arc<dyn ChatModel<()>> = mock;
    let mut profile = ModelProfile::default();
    profile.tool_calling = true;
    profile.parallel_tool_calls = true;
    let tools: Arc<Vec<Box<dyn Tool>>> = Arc::new(vec![Box::new(RunWorkflowTool::new())]);
    let mut history = vec![ChatMessage::user("Triage my inbox.")];

    let result = run_channel_turn_via_graph(
        crate::agent::tinyagents::TurnModelSource::from_model_with_profile(model, profile),
        &mut history,
        tools,
        vec![],
        None,
        "model",
        0.0,
        5,
        MultimodalConfig::default(),
        MultimodalFileConfig::default(),
        None,
    )
    .await
    .expect("orchestrator loop should complete");

    assert_eq!(result.text, "ORCHESTRATOR_DONE");
    let tool_messages = history
        .iter()
        .filter(|message| message.role == "tool")
        .map(|message| message.content.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(tool_messages.contains("DONE") && tool_messages.contains("WORKFLOW_DONE"));
}
