use super::*;
use serde_json::Value;

/// Serialize tests that share the process-global scratch store. Same lock
/// as `todos::ops` — otherwise the two test modules race under `cargo test`'s
/// thread pool.
fn scratch_lock() -> std::sync::MutexGuard<'static, ()> {
    crate::agent::todos::ops::scratch_test_lock()
}

async fn reset_scratch() {
    crate::agent::todos::ops::clear(&TodoScope::Scratch)
        .await
        .expect("clear scratch");
}

fn payload(result: &ToolResult) -> Value {
    serde_json::from_str(&result.output()).expect("json payload")
}

#[tokio::test]
async fn a_write_replaces_the_whole_list_and_a_read_returns_it() {
    let _guard = scratch_lock();
    reset_scratch().await;
    let tool = TodoTool::new();

    let written = tool
        .execute(json!({ "todos": [
            { "content": "Write tests", "status": "in_progress" },
            { "content": "Ship it", "status": "pending" }
        ] }))
        .await
        .unwrap();
    assert!(!written.is_error, "{}", written.output());
    let p = payload(&written);
    assert_eq!(p["todos"].as_array().unwrap().len(), 2);
    assert_eq!(p["todos"][0]["status"], "in_progress");
    assert_eq!(p["todos"][1]["status"], "pending");
    let markdown = p["markdown"].as_str().unwrap();
    assert!(markdown.contains("[~] Write tests"), "{markdown}");
    assert!(markdown.contains("[ ] Ship it"), "{markdown}");

    // Omitting `todos` reads the list back.
    let read = tool.execute(json!({})).await.unwrap();
    assert_eq!(payload(&read)["todos"].as_array().unwrap().len(), 2);

    // The next write is the whole list again, not a patch.
    let rewritten = tool
        .execute(json!({ "todos": [
            { "content": "Write tests", "status": "completed" }
        ] }))
        .await
        .unwrap();
    let p = payload(&rewritten);
    assert_eq!(p["todos"].as_array().unwrap().len(), 1);
    assert_eq!(p["todos"][0]["status"], "completed");
    assert!(p["markdown"].as_str().unwrap().contains("[x] Write tests"));

    // An empty list clears it.
    let cleared = tool.execute(json!({ "todos": [] })).await.unwrap();
    assert!(payload(&cleared)["todos"].as_array().unwrap().is_empty());
    reset_scratch().await;
}

#[tokio::test]
async fn two_in_progress_items_are_rejected() {
    let _guard = scratch_lock();
    reset_scratch().await;
    let result = TodoTool::new()
        .execute(json!({ "todos": [
            { "content": "a", "status": "in_progress" },
            { "content": "b", "status": "in_progress" }
        ] }))
        .await
        .unwrap();
    assert!(result.is_error, "{}", result.output());
    reset_scratch().await;
}

#[tokio::test]
async fn empty_content_and_unknown_status_are_errors() {
    let tool = TodoTool::new();
    let err = tool
        .execute(json!({ "todos": [{ "content": "  ", "status": "pending" }] }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("content"), "{err}");

    let err = tool
        .execute(json!({ "todos": [{ "content": "x", "status": "someday" }] }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("invalid status"), "{err}");
}

#[test]
fn schema_is_the_claude_shape() {
    let tool = TodoTool::new();
    let schema = tool.parameters_schema();
    let props = &schema["properties"];
    assert!(props.get("todos").is_some());
    assert_eq!(
        props.as_object().unwrap().len(),
        1,
        "no per-card ops: {props}"
    );
    assert_eq!(
        props["todos"]["items"]["properties"]["status"]["enum"],
        json!(["pending", "in_progress", "completed"])
    );
    let desc = tool.description();
    assert!(desc.contains("3+ steps"), "missing when-to-use guidance");
    assert!(
        desc.contains("one `in_progress`"),
        "missing single-in_progress rule"
    );
    assert!(
        !desc.contains("board"),
        "the tool must not describe itself as a board"
    );
}

/// The orchestrator's list is its session's list. It used to be routed to one
/// app-wide `orchestrator-tasks` board that nothing rendered, so the items the
/// model wrote never showed up in the thread the user was in.
#[test]
fn every_agent_binds_to_its_own_session() {
    struct ThreadContext(&'static str);
    impl ToolRunContext for ThreadContext {
        fn thread_id(&self) -> Option<&str> {
            Some(self.0)
        }
    }
    let parent = ParentExecutionContext {
        agent_definition_id: "orchestrator".into(),
        allowed_subagent_ids: std::collections::HashSet::new(),
        turn_model_source: crate::agent::tinyagents::TurnModelSource::from_model(Arc::new(
            tinyagents_harness::testkit::ScriptedModel::replies(vec!["done"]),
        )),
        all_tools: Arc::new(Vec::new()),
        all_tool_specs: Arc::new(Vec::new()),
        visible_tool_specs: Arc::new(Vec::new()),
        visible_tool_names: std::collections::HashSet::new(),
        subagent_tool_ceiling_names: std::collections::HashSet::new(),
        model_name: "test-model".into(),
        temperature: 0.0,
        workspace_dir: std::path::PathBuf::from("/tmp/openhuman-todo-parent"),
        workspace_descriptor: None,
        memory: crate::memory::test_support::noop_memory(),
        agent_config: crate::config::AgentConfig::default(),
        workflows: Arc::new(Vec::new()),
        memory_context: Arc::new(None),
        session_id: "orchestrator_thread-live".into(),
        channel: "test".into(),
        connected_integrations: Vec::new(),
        tool_call_format: crate::agent::prompts::ToolCallFormat::Native,
        session_key: "parent-key".into(),
        session_parent_prefix: None,
        on_progress: None,
        run_queue: None,
    };

    assert_eq!(
        current_scope(Some(&parent), Some(&ThreadContext("thread-live"))).session_id(),
        Some("orchestrator_thread-live"),
        "the parent's session wins over the thread id"
    );
    assert_eq!(
        current_scope(None, Some(&ThreadContext("thread-live"))).session_id(),
        Some("thread-live"),
        "a thread-only caller keys on the thread"
    );
    assert_eq!(current_scope(None, None), TodoScope::Scratch);
}

#[tokio::test]
async fn sessions_do_not_see_each_other_and_a_list_survives_across_turns() {
    let a = TodoScope::Session {
        id: "sess-a".into(),
    };
    let b = TodoScope::Session {
        id: "sess-b".into(),
    };
    crate::agent::todos::ops::clear(&a).await.unwrap();
    crate::agent::todos::ops::clear(&b).await.unwrap();

    let item = TodoItem::with_status("only in a", TodoStatus::InProgress);
    crate::agent::todos::ops::replace(&a, vec![item])
        .await
        .unwrap();

    let a_again = crate::agent::todos::ops::list(&a).await.unwrap();
    assert_eq!(
        a_again.items.len(),
        1,
        "a later turn of the same session reads it back"
    );
    assert_eq!(a_again.session_id.as_deref(), Some("sess-a"));
    assert!(crate::agent::todos::ops::list(&b)
        .await
        .unwrap()
        .items
        .is_empty());
}
