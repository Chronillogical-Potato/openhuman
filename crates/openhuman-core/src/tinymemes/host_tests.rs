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

#[test]
fn a_new_message_repeating_an_earlier_one_is_still_appended() {
    // user("hello"), agent("reply"), then a new "hello": a distinct message.
    let messages = vec![msg("u1", "user", "hello"), msg("r1", "agent", "reply")];
    let turns = history_turns(&messages, "hello", |_| false);
    assert_eq!(turns.len(), 3);
    assert_eq!(turns[2].text, "hello");
}

#[test]
fn atomic_writes_replace_the_file_and_leave_no_temp_files() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("slang-index.json");
    write_atomic(&path, "{\"a\":1}").unwrap();
    write_atomic(&path, "{\"a\":2}").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"a\":2}");
    let entries: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert_eq!(entries.len(), 1, "temp files left behind");
}

#[test]
fn an_oversized_remixed_file_is_capped_on_load() {
    let dir = tempfile::TempDir::new().unwrap();
    let ids: Vec<String> = (0..REMIXED_CAP + 50).map(|i| format!("id-{i}")).collect();
    std::fs::write(
        dir.path().join(REMIXED_FILE),
        serde_json::to_string(&ids).unwrap(),
    )
    .unwrap();
    let loaded = load_remixed(dir.path());
    assert_eq!(loaded.len(), REMIXED_CAP);
    // The newest are kept.
    assert_eq!(
        loaded.back().map(String::as_str),
        Some(format!("id-{}", REMIXED_CAP + 49)).as_deref()
    );
    assert_eq!(loaded.front().map(String::as_str), Some("id-50"));
}

#[test]
fn the_fingerprint_follows_the_provider_settings() {
    let config = crate::config::Config::default();
    let base = fingerprint(&config);
    assert_eq!(fingerprint(&config), base);
    let mut changed = config.clone();
    changed.memory_provider = Some("openrouter:deepseek/deepseek-v4-flash".into());
    assert_ne!(fingerprint(&changed), base);
    let mut changed = config.clone();
    changed.inference_url = Some("https://openrouter.ai/api/v1".into());
    assert_ne!(fingerprint(&changed), base);
}
