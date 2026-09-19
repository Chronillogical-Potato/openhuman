use super::*;
use tinyagents_harness::context::RunConfig;

#[tokio::test]
async fn transcript_snapshot_keeps_a_completed_failed_tool_row() {
    let sink = Arc::new(std::sync::Mutex::new(TranscriptSnapshot::default()));
    let middleware = TranscriptSnapshotMiddleware {
        sink: sink.clone(),
        started: Default::default(),
    };
    let mut context = RunContext::new(
        RunConfig::new("snapshot-tool-outcome"),
        crate::agent::tinyagents::host::OpenHumanRunContext::new(),
    );
    let mut call = tinyinference_llm::tool::ToolCall {
        id: "failed-call".into(),
        name: "write_file".into(),
        arguments: serde_json::json!({"path": "blocked.txt"}),
        invalid: None,
    };
    middleware
        .before_tool(&mut context, &(), &mut call)
        .await
        .expect("snapshot accepts tool start");
    let invocation = ToolInvocationIdentity::new("failed-call", "write_file");
    let mut result = TaToolResult::error("permission denied");
    middleware
        .after_tool(&mut context, &(), &invocation, &mut result)
        .await
        .expect("snapshot accepts tool completion");

    let snapshot = sink.lock().expect("snapshot");
    assert_eq!(
        snapshot.messages,
        vec![Message::tool("failed-call", "permission denied")]
    );
    assert_eq!(snapshot.tool_outcomes.len(), 1);
    let outcome = &snapshot.tool_outcomes[0];
    assert_eq!(outcome.call_id, "failed-call");
    assert_eq!(outcome.name, "write_file");
    assert_eq!(
        outcome.arguments,
        serde_json::json!({"path": "blocked.txt"})
    );
    assert!(!outcome.success);
    assert_eq!(outcome.content, "permission denied");
}
