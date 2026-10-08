//! Sub-agents belong to the agent that declared them: only it gets the
//! delegation tool, the delegated work runs on that agent's provider, and the
//! built-in catalogue ids cannot be taken as agent ids.

mod common;

use common::{
    chat_requests, offline_config, route, runtime, scripted_provider, stub_backend,
    tool_call_completion, tool_names,
};
use openhuman_embed::{
    Access, AgentDefinitionSpec, AgentError, AgentSpec, Runtime, ToolScopeSpec, Workspace,
};

const HELPER_MARK: &str = "HELPER-SUBAGENT-MARK";

#[test]
fn sub_agents_stay_with_the_agent_that_declared_them() {
    let _ = env_logger::builder().is_test(true).try_init();
    runtime().block_on(async {
        tokio::spawn(async move {
            let backend = stub_backend().await;
            let runtime_provider = scripted_provider(Vec::new(), "runtime-provider").await;
            let runtime = Runtime::builder()
                .config(offline_config())
                .workspace(Workspace::Ephemeral)
                .backend_url(backend.uri())
                .provider(route(&runtime_provider, "runtime-model"))
                .access(Access::full())
                .build()
                .await
                .expect("runtime builds");

            // ── reserved ids ──
            for reserved in ["orchestrator", "planner", "summarizer"] {
                let refused = runtime.agent(AgentSpec::new(reserved));
                assert!(
                    matches!(refused, Err(AgentError::ReservedId(ref id)) if id == reserved),
                    "{reserved} must be reserved"
                );
            }
            let refused = runtime.agent(
                AgentSpec::new("lead-x").subagents([("critic", AgentDefinitionSpec::new())]),
            );
            assert!(matches!(refused, Err(AgentError::ReservedId(_))));

            // ── A declares a helper; B does not ──
            let a_provider = scripted_provider(
                vec![tool_call_completion(
                    "delegate_a_helper",
                    &serde_json::json!({ "prompt": "summarise the plan" }).to_string(),
                )],
                "a-done",
            )
            .await;
            let b_provider = scripted_provider(Vec::new(), "b-done").await;
            let a = runtime
                .agent(
                    AgentSpec::new("lead-a")
                        .provider(route(&a_provider, "a-model"))
                        .access(Access::full())
                        .definition(
                            AgentDefinitionSpec::new().tools(ToolScopeSpec::Named(Vec::new())),
                        )
                        .subagents([(
                            "a_helper",
                            AgentDefinitionSpec::new()
                                .system_prompt(format!("You are {HELPER_MARK}."))
                                .when_to_use("Summarises plans for lead-a.")
                                .tools(ToolScopeSpec::Named(Vec::new())),
                        )]),
                )
                .expect("a instantiates");
            let b = runtime
                .agent(
                    AgentSpec::new("plain-b")
                        .provider(route(&b_provider, "b-model"))
                        .access(Access::full())
                        .definition(
                            AgentDefinitionSpec::new().tools(ToolScopeSpec::Named(Vec::new())),
                        ),
                )
                .expect("b instantiates");

            let a_out = a.run("plan something").await.expect("a's turn returns");
            assert!(a_out.reply.contains("a-done"), "{}", a_out.reply);
            b.run("plan something").await.expect("b's turn returns");

            let a_requests = chat_requests(&a_provider).await;
            assert!(
                tool_names(&a_requests[0])
                    .iter()
                    .any(|t| t == "delegate_a_helper"),
                "a is offered its own helper: {:?}",
                tool_names(&a_requests[0])
            );
            assert!(
                a_requests
                    .iter()
                    .filter(|r| !String::from_utf8_lossy(&r.body).contains(HELPER_MARK))
                    .any(|r| common::tool_results(r).contains("a_helper")),
                "the delegation was accepted for a's helper"
            );
            let mut helper_ran_on_a = false;
            for _ in 0..1000 {
                helper_ran_on_a = chat_requests(&a_provider)
                    .await
                    .iter()
                    .any(|r| String::from_utf8_lossy(&r.body).contains(HELPER_MARK));
                if helper_ran_on_a {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            assert!(
                helper_ran_on_a,
                "the detached helper turn runs on a's provider, under a's context"
            );

            let b_requests = chat_requests(&b_provider).await;
            assert!(!b_requests.is_empty());
            for request in &b_requests {
                assert!(
                    !tool_names(request).iter().any(|t| t == "delegate_a_helper"),
                    "b never sees a's helper"
                );
                assert!(!String::from_utf8_lossy(&request.body).contains(HELPER_MARK));
            }
            assert_eq!(
                chat_requests(&runtime_provider).await.len(),
                0,
                "no delegated work falls back to the runtime's provider"
            );
        })
        .await
        .expect("test task");
    });
}
