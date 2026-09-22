use tinytools_jev::JevOption;

use super::*;

fn request(instructions: Option<&str>) -> JevRequest {
    JevRequest {
        intent: "ping alex".into(),
        recent_turns: vec!["earlier turn".into()],
        options: vec![
            JevOption {
                key: "SLACK_SEND_MESSAGE".into(),
                description: "send a message (from slack)".into(),
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
fn builds_one_choice_and_one_noul_from_the_request() {
    let wire = TinyJevEvaluator::build(&request(None));
    assert_eq!(wire.model, "jev-latest");
    assert_eq!(wire.state["request"], "ping alex");
    assert_eq!(wire.state["recent_user_turns"][0], "earlier turn");
    assert_eq!(wire.questions.len(), 2);
    match wire.questions.get("tool") {
        Some(Question::Choice(choice)) => {
            assert_eq!(choice.criteria.len(), 2);
            assert!(choice.instructions.as_str().unwrap().starts_with("Which tool"));
        }
        other => panic!("expected a choice, got {other:?}"),
    }
    assert!(matches!(
        wire.questions.get("needs_tool"),
        Some(Question::Noul(_))
    ));
}

#[test]
fn request_instructions_replace_the_default_wording() {
    let wire = TinyJevEvaluator::build(&request(Some("Which group of tools applies?")));
    match wire.questions.get("tool") {
        Some(Question::Choice(choice)) => {
            assert_eq!(choice.instructions, json!("Which group of tools applies?"));
        }
        other => panic!("expected a choice, got {other:?}"),
    }
}

#[test]
fn failures_map_without_leaking_the_key() {
    let failure = EvaluationFailure {
        error: JevError::Authentication,
        attempts: 2,
        latency: Duration::from_millis(5),
    };
    let text = map_failure(failure).to_string();
    assert!(text.contains("authentication failed"), "{text}");
    assert!(text.contains("2 attempt(s)"), "{text}");
    assert!(matches!(
        map_failure(EvaluationFailure {
            error: JevError::Timeout,
            attempts: 1,
            latency: Duration::ZERO,
        }),
        RankError::Timeout
    ));
}
