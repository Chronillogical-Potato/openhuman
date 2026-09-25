//! Tests for the turn prelude's tool surface across a resumed thread.

use std::sync::Arc;

use tinyagents_runtime::ToolSnapshot;
use tinytools::ToolSpec;

fn spec(name: &str) -> ToolSpec {
    ToolSpec {
        name: name.into(),
        description: format!("{name} description"),
        parameters: serde_json::json!({ "type": "object", "properties": {} }),
    }
}

/// The incident this guards: a thread resumed in a fresh process (empty
/// integrations cache) lost every Composio action, so the orchestrator's
/// `tool_search` had nothing to find although its restored prompt told it to
/// search for the Gmail action. Rehydration is permitted only after the
/// current integration authorization snapshot confirms Gmail is connected.
#[tokio::test]
async fn a_resumed_orchestrator_keeps_the_integration_actions_it_was_sent() {
    let _ = crate::agent::harness::definition::AgentDefinitionRegistry::init_global_builtins();
    let action_dir = tempfile::tempdir().expect("tempdir");
    let model: Arc<dyn tinyinference_llm::model::ChatModel<()>> =
        Arc::new(tinyagents_harness::testkit::ScriptedModel::new(Vec::new()));
    let mut host = crate::agent::SessionHostBuilder::new()
        .chat_model(model)
        .tools(Vec::new())
        .action_dir(action_dir.path().to_path_buf())
        .memory(crate::memory::test_support::noop_memory())
        .tool_dispatcher(Box::new(tinytools_agent::dialect::XmlDialect))
        .agent_definition_name("orchestrator")
        .build()
        .expect("session build");
    host.ensure_runtime_session().expect("runtime session");

    let state = host
        .runtime_state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let prelude = state.prelude.as_ref().expect("prelude");

    // Fresh process: no integrations known, so no actions are synthesised.
    prelude.refresh_delegation_tool_surface();
    assert!(!prelude
        .synthesized_tool_names_for_test()
        .contains("GMAIL_SEND_EMAIL"));

    // Resume hands back what the thread was sent.
    let recorded = ToolSnapshot::new(vec![
        spec("GMAIL_SEND_EMAIL"),
        spec("GMAIL_FETCH_EMAILS"),
        spec("web_fetch"),
    ])
    .expect("snapshot");
    prelude.adopt_recorded_tools(Some(&recorded));
    {
        let mut mutable = prelude.mutable.lock().expect("prelude state");
        mutable.connected_integrations = vec![crate::agent::prompts::ConnectedIntegration {
            toolkit: "gmail".into(),
            description: String::new(),
            tools: Vec::new(),
            gated_tools: Vec::new(),
            connected: true,
            connections: Vec::new(),
            non_active_status: None,
        }];
        mutable.connected_integrations_authoritative = true;
    }
    prelude.refresh_delegation_tool_surface();

    let names = prelude.synthesized_tool_names_for_test();
    assert!(names.contains("GMAIL_SEND_EMAIL"), "{names:?}");
    assert!(names.contains("GMAIL_FETCH_EMAILS"), "{names:?}");
    assert!(
        !names.contains("web_fetch"),
        "only integration actions are rebuilt from the record"
    );
}

/// The incident this guards: `session_locator()` is called from more than one
/// place while assembling a session's runtime turn machinery (the
/// `before_resume` resume target and the eager construction-time
/// `builder.session(...)` bind), and tinyagents only accepts a later
/// transcript-target change when it is the exact same locator object
/// (`Arc::ptr_eq`), not merely an equivalent one over the same file. Before
/// this was memoized, each call minted a fresh `FileTranscriptLocator`, so a
/// thread's second turn was rejected with "cannot change a transcript target
/// after it is bound or committed" even though both binds agreed on the
/// destination. Pin the fix directly: every call must return the identical
/// `Arc`.
#[tokio::test]
async fn session_locator_is_memoized_across_calls() {
    let action_dir = tempfile::tempdir().expect("tempdir");
    let model: Arc<dyn tinyinference_llm::model::ChatModel<()>> =
        Arc::new(tinyagents_harness::testkit::ScriptedModel::new(Vec::new()));
    let host = crate::agent::SessionHostBuilder::new()
        .chat_model(model)
        .tools(Vec::new())
        .action_dir(action_dir.path().to_path_buf())
        .memory(crate::memory::test_support::noop_memory())
        .tool_dispatcher(Box::new(tinytools_agent::dialect::XmlDialect))
        .agent_definition_name("orchestrator")
        .build()
        .expect("session build");

    let first = host.session_locator();
    let second = host.session_locator();
    assert!(
        Arc::ptr_eq(&first, &second),
        "session_locator() must return the same Arc on every call, or tinyagents' \
         same-binding check rejects the second transcript bind"
    );
}
