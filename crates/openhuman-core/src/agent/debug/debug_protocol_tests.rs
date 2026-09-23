use super::*;

#[test]
fn text_mode_dispatcher_instructions_follow_the_available_catalogue() {
    assert!(text_mode_dispatcher_instructions(&[]).is_empty());

    let schema = tinyinference_llm::tool::ToolSchema::new(
        "calendar_search",
        "Search calendar events.",
        serde_json::json!({"type": "object", "properties": {}}),
    );
    let instructions = text_mode_dispatcher_instructions(&[schema]);
    assert!(instructions.contains("## Tool Use Protocol"));
    assert!(instructions.contains("calendar_search"));
}
