use super::*;

fn sidecar(
) -> std::sync::Arc<std::sync::Mutex<crate::agent::tinyagents::host::run_context::SessionTurnSidecar>>
{
    crate::agent::tinyagents::host::OpenHumanRunContext::new()
        .session_sidecar
        .clone()
}

#[test]
fn graph_failure_persists_only_accepted_snapshot_history() {
    let snapshot = Arc::new(std::sync::Mutex::new(
        crate::agent::tinyagents::TranscriptSnapshot {
            messages: vec![
                Message::system("stable prefix"),
                Message::user("request"),
                Message::assistant("provider reply not yet accepted by a later request"),
            ],
            accepted_len: 2,
            request_base_len: 2,
            ..Default::default()
        },
    ));
    let failure = driver_error_with_snapshot(
        "provider rejected follow-up",
        &snapshot,
        &sidecar(),
        std::time::Duration::from_millis(1),
        "chat-v1",
    );
    let partial = failure.partial.expect("snapshot should produce a partial");
    assert_eq!(partial.history.len(), 2);
    assert_eq!(partial.history[0].text(), "stable prefix");
    assert_eq!(partial.history[1].text(), "request");
    assert!(partial
        .partial
        .expect("display partial")
        .content
        .contains("provider rejected follow-up"));
}

#[test]
fn graph_failure_copies_snapshot_usage_and_failed_tool_outcome_to_sidecar() {
    let snapshot = Arc::new(std::sync::Mutex::new(
        crate::agent::tinyagents::TranscriptSnapshot {
            messages: vec![Message::user("request"), Message::tool("call-1", "denied")],
            accepted_len: 1,
            request_base_len: 1,
            input_tokens: 21,
            output_tokens: 8,
            cached_input_tokens: 3,
            charged_amount_usd: 0.004,
            resolved_route: Some(tinyinference_llm::model::ResolvedModelRoute::new(
                "openhuman",
                "chat-concrete",
                "chat-v1",
            )),
            pricing_model: None,
            model_calls: 2,
            tool_outcomes: vec![crate::agent::tinyagents::ToolCallOutcome {
                call_id: "call-1".into(),
                name: "write_file".into(),
                arguments: serde_json::json!({"path": "blocked.txt"}),
                success: false,
                content: "denied".into(),
                duration_ms: 17,
            }],
        },
    ));
    let sidecar = sidecar();
    let failure = driver_error_with_snapshot(
        "tool follow-up was rejected",
        &snapshot,
        &sidecar,
        std::time::Duration::from_millis(25),
        "chat-v1",
    );
    let partial = failure.partial.expect("recoverable snapshot partial");
    assert_eq!(partial.history, vec![Message::user("request")]);
    let observed = sidecar.lock().expect("sidecar");
    assert_eq!(
        (
            observed.model_calls,
            observed.input_tokens,
            observed.output_tokens,
            observed.cached_input_tokens
        ),
        (2, 21, 8, 3)
    );
    assert!((observed.cost_usd - 0.004).abs() < f64::EPSILON);
    let route = observed
        .resolved_route
        .as_ref()
        .expect("accepted route reaches sidecar");
    assert_eq!(
        (
            route.provider.as_str(),
            route.model.as_str(),
            route.route.as_str()
        ),
        ("openhuman", "chat-concrete", "chat-v1")
    );
    assert_eq!(observed.tool_calls, 1);
    assert_eq!(
        observed.duration,
        Some(std::time::Duration::from_millis(25))
    );
    assert_eq!(observed.tool_outcomes.len(), 1);
    let failure = &observed.tool_outcomes[0];
    assert!(
        !failure.success,
        "completed failure state survives graph error"
    );
    assert_eq!(failure.content, "denied");
    assert_eq!(
        failure.arguments,
        serde_json::json!({"path": "blocked.txt"})
    );
    assert_eq!(failure.duration_ms, 17);
}

#[test]
fn tool_snapshot_with_no_executable_source_fails_closed_before_graph() {
    let visible = HashSet::from(["revoked_tool".to_string()]);
    let error = ensure_snapshot_tools_are_executable(
        &visible,
        &Arc::new(Vec::new()),
        &Arc::new(Vec::new()),
    )
    .expect_err("a declared tool must have a request-scoped executable source");
    assert!(error.error.to_string().contains("revoked_tool"));
}
