use super::*;

#[test]
fn transcript_adapter_round_trips_cache_breakpoints() {
    let original = ChatMessage {
        id: Some("message-1".to_string()),
        role: "system".to_string(),
        content: "system prompt".to_string(),
        extra_metadata: None,
        cache_breakpoints: vec![3, 7],
    };

    let durable = crate::agent::messages::transcript_message_from_chat(&original);
    assert_eq!(durable.cache_breakpoints, vec![3, 7]);

    let restored = crate::agent::messages::chat_message_from_transcript(durable);
    assert_eq!(restored.id, original.id);
    assert_eq!(restored.role, original.role);
    assert_eq!(restored.content, original.content);
    assert_eq!(restored.cache_breakpoints, original.cache_breakpoints);
}
