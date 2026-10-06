use super::*;

use tinyagents_harness::summarization::{checkpoint_message, SummaryPlacement};
use tinyinference_llm::ToolCall;

fn assistant_with_call(id: &str, name: &str) -> Message {
    let mut message = Message::assistant("");
    if let Message::Assistant(assistant) = &mut message {
        assistant
            .tool_calls
            .push(ToolCall::new(id, name, serde_json::json!({})));
    }
    message
}

#[test]
fn without_a_checkpoint_the_whole_thread_is_in_the_prompt() {
    let history = vec![
        Message::user("one"),
        Message::assistant("a"),
        Message::user("two"),
    ];
    assert_eq!(in_prompt_window(&history, 2, None), (0, false));
}

#[test]
fn after_a_checkpoint_the_window_starts_at_the_first_kept_turn() {
    let current = Message::user("five");
    let history = vec![
        Message::system("prompt"),
        checkpoint_message(SummaryPlacement::User, "earlier turns"),
        Message::user("three"),
        Message::assistant("c"),
        Message::user("four"),
        Message::assistant("d"),
        current.clone(),
    ];
    // Five turns committed before this one; turns 3 and 4 are verbatim, so
    // the window opens at turn 3, user index 6.
    assert_eq!(in_prompt_window(&history, 5, Some(&current)), (6, true));
    assert_eq!(
        in_prompt_window(&history[..6], 5, None),
        (6, true),
        "the request input is not counted whether or not it was appended"
    );
}

#[test]
fn committed_tool_calls_pair_each_call_with_its_result() {
    let history = vec![
        Message::user("earlier"),
        assistant_with_call("old", "ignored"),
        Message::user("find the file"),
        assistant_with_call("c1", "glob"),
        Message::tool("c1", "src/main.rs"),
        assistant_with_call("c2", "noop"),
        Message::assistant("Found it."),
    ];
    let calls = committed_tool_calls(&history);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].name, "glob");
    assert_eq!(calls[0].id.as_deref(), Some("c1"));
    assert_eq!(calls[0].result.as_deref(), Some("src/main.rs"));
    assert_eq!(calls[1].result, None);
}
