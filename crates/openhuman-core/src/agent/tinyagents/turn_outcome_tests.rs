use super::*;
use tinyinference_llm::message::{AssistantMessage, ContentBlock};
use tinyinference_llm::model::ModelResponse;

fn response(finish_reason: &str, text: &str) -> ModelResponse {
    let content = if text.is_empty() {
        vec![ContentBlock::Thinking {
            text: "still weighing the approach".into(),
            signature: None,
        }]
    } else {
        vec![ContentBlock::Text(text.to_string())]
    };
    ModelResponse {
        message: AssistantMessage {
            id: None,
            content,
            tool_calls: Vec::new(),
            usage: None,
            origin: None,
        },
        usage: None,
        finish_reason: Some(finish_reason.to_string()),
        raw: None,
        resolved_model: None,
        continue_turn: None,
        served_from_cache: false,
        correlation: None,
        resolved_route: None,
    }
}

#[test]
fn a_length_stop_with_only_reasoning_ran_out_of_output_budget() {
    assert!(ended_out_of_output_budget(Some(&response("length", ""))));
}

#[test]
fn a_length_stop_with_visible_text_is_a_real_answer() {
    assert!(!ended_out_of_output_budget(Some(&response(
        "length",
        "partial answer"
    ))));
}

#[test]
fn a_normal_blank_stop_or_no_final_response_is_not_truncation() {
    assert!(!ended_out_of_output_budget(Some(&response("stop", ""))));
    assert!(!ended_out_of_output_budget(None));
}
