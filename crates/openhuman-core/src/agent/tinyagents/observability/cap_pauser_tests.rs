use super::*;

use async_trait::async_trait;
use tinyagents_harness::context::{RunConfig, RunContext};
use tinyagents_harness::error::Result as TaResult;
use tinyagents_harness::events::EventSink;
use tinyagents_harness::middleware::Middleware;
use tinyagents_harness::runtime::{AgentHarness, RunPolicy};
use tinyagents_harness::steering::SteeringPolicy;
use tinyagents_harness::subagent::SubAgent;
use tinyagents_harness::testkit::{FakeTool, ScriptedModel};
use tinyagents_harness::tool::ToolResult;
use tinyinference::message::{AssistantMessage, Message};
use tinyinference::model::ModelResponse;
use tinyinference::tool::ToolCall;

const CAP: usize = 3;

fn tool_call_response(id: &str) -> ModelResponse {
    ModelResponse {
        message: AssistantMessage {
            id: Some(format!("msg-{id}")),
            content: Vec::new(),
            tool_calls: vec![ToolCall::new(id, "lookup", serde_json::json!({}))],
            usage: None,
        },
        usage: None,
        finish_reason: Some("tool_calls".to_string()),
        raw: None,
        resolved_model: None,
        continue_turn: None,
        served_from_cache: false,
    }
}

/// Runs a one-call nested sub-agent on the parent's sink after every tool, the
/// way the payload summarizer does for an oversized result. The child is named
/// after the parent run so its run id (`agent_turn-d1-N`) shares the parent's
/// prefix — attribution must still tell the two apart.
struct NestedRunAfterEveryTool;

#[async_trait]
impl Middleware<()> for NestedRunAfterEveryTool {
    fn name(&self) -> &str {
        "nested_run_after_every_tool"
    }

    async fn after_tool(
        &self,
        ctx: &mut RunContext<()>,
        _state: &(),
        _result: &mut ToolResult,
    ) -> TaResult<()> {
        let mut child: AgentHarness<()> = AgentHarness::new();
        child
            .register_model("child", Arc::new(ScriptedModel::replies(vec!["summary"])))
            .set_default_model("child");
        SubAgent::new("agent_turn", "nested", Arc::new(child))
            .invoke_with_events(&(), (), ctx.depth(), "summarize", &ctx.events)
            .await?;
        Ok(())
    }
}

#[tokio::test]
async fn nested_runs_on_the_shared_sink_do_not_spend_the_parents_cap() {
    let mut harness: AgentHarness<()> = AgentHarness::new();
    let mut policy = RunPolicy::default();
    // Hard ceiling well above the cap, so only the pauser can stop the run.
    policy.limits.max_model_calls = 10 * CAP;
    policy.limits.max_tool_calls = 80 * CAP;
    harness.with_policy(policy);
    let replies = (1..=10 * CAP)
        .map(|i| tool_call_response(&format!("t{i}")))
        .collect();
    harness
        .register_model("parent", Arc::new(ScriptedModel::new(replies)))
        .set_default_model("parent");
    harness.register_tool(Arc::new(FakeTool::returning("lookup", "ok")));
    harness.push_middleware(Arc::new(NestedRunAfterEveryTool));

    let sink = EventSink::new();
    let handle = SteeringHandle::new(SteeringPolicy::allow_all());
    let ctx = RunContext::new(RunConfig::new("agent_turn"), ())
        .with_events(sink.clone())
        .with_steering(handle.clone());
    sink.subscribe(CapPauser::new(handle, CAP, ctx.run_id().as_str(), None));

    let run = harness
        .invoke_in_context(&(), ctx, vec![Message::user("go")])
        .await
        .expect("run completes");

    assert!(run.paused.is_some(), "the cap pauser must pause the run");
    assert_eq!(
        run.model_calls, CAP,
        "the pause must land after the run's OWN {CAP} model calls; a nested run's \
         completions on the shared sink must not count toward the parent's cap"
    );
}
