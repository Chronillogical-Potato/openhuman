use super::*;
use tinyinference_llm::usage::Usage;

fn response(finish: &str, reasoning: u64, cached: bool) -> ModelResponse {
    let mut response = ModelResponse::assistant("{}")
        .with_usage(Usage {
            reasoning_tokens: reasoning,
            ..Default::default()
        })
        .with_finish_reason(finish);
    response.served_from_cache = cached;
    response
}

#[test]
fn the_last_call_reports_and_reasoning_adds_up() {
    let scope = ResponseShapeScope::new(ResponseShape::default());
    record(&scope, &response("tool_calls", 3, false));
    record(&scope, &response("stop", 4, false));
    record(&scope, &response("stop", 100, true));

    let report = scope.report();
    assert_eq!(report.finish_reason.as_deref(), Some("stop"));
    assert_eq!(report.reasoning_tokens, 7, "a cache replay spent nothing");
}

#[tokio::test]
async fn install_needs_a_scoped_shape_and_a_root_turn() {
    let mut harness: AgentHarness<(), OpenHumanRunContext> = AgentHarness::new();
    install(&mut harness, true);
    let scope = ResponseShapeScope::new(ResponseShape {
        max_output_tokens: Some(64),
        ..Default::default()
    });
    with_response_shape(scope, async {
        install(&mut harness, false);
        install(&mut harness, true);
    })
    .await;
}
