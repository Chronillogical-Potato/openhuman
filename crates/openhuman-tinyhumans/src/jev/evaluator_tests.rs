use std::collections::BTreeMap;

use tinyjevclient::{Answer, ChoiceAnswer, ClientConfig, EvaluationResponse, NoulAnswer, Usage};
use tinytools::RankError;
use tinytools_jev::{JevEvaluator, JevOption, JevRequest};

use super::*;

fn request(instructions: Option<&str>) -> JevRequest {
    JevRequest {
        intent: "ping alex on slack".into(),
        recent_turns: vec!["hi".into()],
        options: vec![
            JevOption {
                key: "slack_send".into(),
                description: "Send a Slack message (from slack)".into(),
            },
            JevOption {
                key: "none".into(),
                description: "No listed tool accomplishes the request.".into(),
            },
        ],
        model: "jev-latest".into(),
        instructions: instructions.map(str::to_owned),
    }
}

#[test]
fn wire_request_carries_every_option_the_state_and_both_questions() {
    let wire = SystemOneEvaluator::build_request(&request(None)).expect("valid");
    assert_eq!(wire.model, "jev-latest");
    assert_eq!(wire.state["request"], "ping alex on slack");
    assert_eq!(wire.state["recent_user_turns"][0], "hi");
    let Some(Question::Choice(choice)) = wire.questions.get(TOOL_QUESTION) else {
        panic!("tool question is a choice");
    };
    assert_eq!(choice.criteria.len(), 2);
    assert!(choice.criteria.contains_key("none"));
    assert_eq!(choice.instructions, json!(DEFAULT_INSTRUCTIONS));
    assert!(matches!(
        wire.questions.get(NEEDS_TOOL_QUESTION),
        Some(Question::Noul(_))
    ));
}

#[test]
fn ranker_instructions_replace_the_default_wording() {
    let wire = SystemOneEvaluator::build_request(&request(Some("Which group applies?")))
        .expect("valid");
    let Some(Question::Choice(choice)) = wire.questions.get(TOOL_QUESTION) else {
        panic!("tool question is a choice");
    };
    assert_eq!(choice.instructions, json!("Which group applies?"));
}

#[test]
fn duplicate_option_keys_are_invalid_input() {
    let mut duplicated = request(None);
    duplicated.options.push(JevOption {
        key: "slack_send".into(),
        description: "again".into(),
    });
    match SystemOneEvaluator::build_request(&duplicated) {
        Err(RankError::InvalidInput { reason }) => assert!(reason.contains("slack_send")),
        other => panic!("expected invalid input, got {other:?}"),
    }
}

#[test]
fn client_config_errors_surface_as_invalid_input() {
    let err = SystemOneEvaluator::from_config(ClientConfig::new("")).expect_err("empty key");
    assert!(matches!(err, RankError::InvalidInput { .. }), "{err}");
}

#[test]
fn failures_map_without_leaking_the_client_error_source() {
    let backend = map_failure(EvaluationFailure {
        error: JevError::RateLimited,
        attempts: 3,
        latency: Duration::ZERO,
    });
    match backend {
        RankError::Backend { reason } => assert!(reason.contains("3 attempt(s)"), "{reason}"),
        other => panic!("expected backend, got {other}"),
    }
    assert!(matches!(
        map_failure(EvaluationFailure {
            error: JevError::Timeout,
            attempts: 1,
            latency: Duration::ZERO,
        }),
        RankError::Timeout
    ));
}

/// The evaluator answers with exactly the probability table the ranker
/// decodes; no reshaping happens on the way back.
#[tokio::test]
async fn a_choice_answer_becomes_a_decision() {
    let response = EvaluationResponse {
        model: "jev-latest".into(),
        answers: BTreeMap::from([
            (
                TOOL_QUESTION.to_owned(),
                Answer::Choice(ChoiceAnswer {
                    choice: "slack_send".into(),
                    probabilities: BTreeMap::from([
                        ("slack_send".to_owned(), 0.9),
                        ("none".to_owned(), 0.1),
                    ]),
                    confidence: 0.9,
                }),
            ),
            (
                NEEDS_TOOL_QUESTION.to_owned(),
                Answer::Noul(NoulAnswer { noul: 0.95 }),
            ),
        ]),
        usage: Usage {
            input_tokens: Some(1200),
            output_tokens: None,
        },
    };
    let server = spawn_system_one(response).await;
    let mut config = ClientConfig::new("test-key");
    config.base_url = server.url.clone();
    let evaluator = SystemOneEvaluator::from_config(config).expect("client");
    let decision = evaluator.evaluate(&request(None)).await.expect("decision");
    assert_eq!(decision.probabilities["slack_send"], 0.9);
    assert_eq!(decision.choice_confidence, 0.9);
    assert_eq!(decision.needs_tool, Some(0.95));
    assert_eq!(decision.input_tokens, Some(1200));
    assert_eq!(decision.attempts, 1);
    server.shutdown.send(()).ok();
}

struct SystemOneStub {
    url: String,
    shutdown: tokio::sync::oneshot::Sender<()>,
}

/// One-route System One stand-in answering every evaluation with `response`.
async fn spawn_system_one(response: EvaluationResponse) -> SystemOneStub {
    use axum::{Router, routing::post};
    let body = serde_json::to_value(&response).expect("serialise");
    let app = Router::new().fallback(post(move || {
        let body = body.clone();
        async move { axum::Json(body) }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let (shutdown, rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                rx.await.ok();
            })
            .await
            .ok();
    });
    SystemOneStub {
        url: format!("http://{addr}"),
        shutdown,
    }
}
