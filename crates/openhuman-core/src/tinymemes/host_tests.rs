use super::*;

fn msg(id: &str, sender: &str, content: &str) -> ConversationMessage {
    ConversationMessage {
        id: id.to_owned(),
        content: content.to_owned(),
        message_type: "text".to_owned(),
        extra_metadata: serde_json::Value::Null,
        sender: sender.to_owned(),
        created_at: "2026-10-09T00:00:00Z".to_owned(),
    }
}

#[test]
fn history_marks_remixed_replies_and_appends_the_pending_message() {
    let messages = vec![
        msg("u1", "user", "bhai build fail"),
        msg("r1", "agent", "Arre yaar, moye moye"),
        msg("r2", "agent", "Plain reply"),
        msg("s1", "system", "ignored"),
        msg("u2", "user", "  "),
    ];
    let turns = history_turns(&messages, "phir se fail", |id| id == "r1");
    assert_eq!(turns.len(), 4);
    assert!(turns[1].remixed);
    assert!(!turns[2].remixed);
    assert_eq!(turns[3].text, "phir se fail");
}

#[test]
fn pending_message_is_not_duplicated_when_already_stored() {
    let messages = vec![msg("u1", "user", "hello there")];
    let turns = history_turns(&messages, "hello there ", |_| false);
    assert_eq!(turns.len(), 1);
}
