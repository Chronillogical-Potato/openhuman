use super::*;

/// Answers two tool rounds, then rejects the third request with a 400 (which the
/// harness does not retry).
struct FailsOnThirdCallProvider {
    calls: AtomicUsize,
}
#[async_trait]
impl ChatModel<()> for FailsOnThirdCallProvider {
    fn profile(&self) -> Option<&ModelProfile> {
        Some(native_tool_profile())
    }

    async fn invoke(
        &self,
        _state: &(),
        _request: ModelRequest,
    ) -> tinyinference::Result<ModelResponse> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        if n < 2 {
            Ok(tool_response(
                &format!("call-{n}"),
                "echo",
                serde_json::json!({ "msg": format!("round-{n}") }),
            ))
        } else {
            Err(tinyinference::Error::Model(
                "400 Bad Request: provider boom".to_string(),
            ))
        }
    }
}

/// #6281: a failed sub-agent run persists only the round a provider accepted as
/// structured history; the round only the rejected request carried goes into the
/// failure marker as text, so a resumed sub-agent cannot replay it.
#[tokio::test]
async fn failed_subagent_run_keeps_its_unanswered_round_out_of_history() {
    let provider = Arc::new(FailsOnThirdCallProvider {
        calls: AtomicUsize::new(0),
    });
    let parent_tools: Arc<Vec<Box<dyn Tool>>> = Arc::new(vec![Box::new(EchoTool)]);
    let mut allowed = HashSet::new();
    allowed.insert("echo".to_string());
    let mut history = vec![ChatMessage::user("please echo twice")];
    let workspace = tempfile::TempDir::new().expect("temp workspace");
    let stem = "root-session__failed_run";

    let result = run_subagent_via_graph(
        crate::agent::tinyagents::TurnModelSource::from_model(provider),
        "mock-model",
        0.0,
        &mut history,
        parent_tools,
        vec![],
        vec![],
        allowed,
        10,
        None,
        None,
        "researcher",
        "task-failed",
        false,
        None,
        workspace.path().to_path_buf(),
        None,
        1024,
        false,
        stem,
        "mock-channel",
        None,
        AgentTokenjuiceCompression::Off,
        None,
    )
    .await;
    assert!(result.is_err(), "the rejected third call fails the run");

    use crate::agent::harness::session::transcript;
    let path =
        transcript::resolve_keyed_transcript_path(workspace.path(), stem).expect("transcript path");
    let persisted = transcript::read_transcript(&path).expect("failed run transcript");
    let tool_rows: Vec<&str> = persisted
        .messages
        .iter()
        .filter(|message| message.role == "tool")
        .map(|message| message.content.as_str())
        .collect();
    assert_eq!(
        tool_rows.len(),
        1,
        "only the accepted round is persisted as a tool row: {tool_rows:?}"
    );
    assert!(
        tool_rows[0].contains("echoed:round-0"),
        "got: {tool_rows:?}"
    );
    let marker = &persisted.messages.last().expect("failure marker").content;
    assert!(
        marker.starts_with("[subagent run failed before completion:")
            && marker.contains("provider boom")
            && marker.contains("round-1"),
        "the marker carries the cause and the unanswered round as text, got: {marker}"
    );
}
