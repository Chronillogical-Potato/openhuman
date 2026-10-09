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

fn config_in(dir: &std::path::Path) -> Config {
    let mut config = Config::default();
    config.config_path = dir.join("config.toml");
    config.workspace_dir = dir.join("workspace");
    config.secrets.encrypt = false;
    config
}

#[test]
fn remixed_ids_are_deduplicated_persisted_and_capped() {
    let dir = tempfile::TempDir::new().unwrap();
    let host = host_for(&config_in(dir.path())).expect("engine");
    host.mark_remixed("m1".into());
    host.mark_remixed("m1".into());
    host.mark_remixed("m2".into());
    assert!(host.is_remixed("m1") && host.is_remixed("m2"));
    assert!(!host.is_remixed("m3"));
    let saved = load_remixed(&dir.path().join("workspace").join(DIR));
    assert_eq!(saved, ["m1", "m2"]);

    let mut ids: VecDeque<String> = (0..REMIXED_CAP + 3).map(|i| i.to_string()).collect();
    cap_remixed(&mut ids);
    assert_eq!(ids.len(), REMIXED_CAP);
    assert_eq!(ids.front().map(String::as_str), Some("3"));
}

#[test]
fn learned_state_survives_an_engine_rebuild() {
    let dir = tempfile::TempDir::new().unwrap();
    let config = config_in(dir.path());
    let first = host_for(&config).expect("engine");
    assert!(Arc::ptr_eq(&first, &host_for(&config).unwrap()));
    first.mark_remixed("kept".into());

    // A settings change rebuilds the engine but keeps what it learned.
    let mut changed = config.clone();
    changed.inference_url = Some("https://openrouter.ai/api/v1".into());
    let rebuilt = host_for(&changed).expect("engine");
    assert!(!Arc::ptr_eq(&first, &rebuilt));
    assert!(rebuilt.is_remixed("kept"));
}

#[test]
fn indexes_round_trip_through_disk() {
    let dir = tempfile::TempDir::new().unwrap();
    let config = config_in(dir.path());
    let host = host_for(&config).expect("engine");
    host.save_index();
    host.save_memes();
    let state = dir.path().join("workspace").join(DIR);
    assert!(state.join(INDEX_FILE).exists());
    assert!(state.join(MEME_INDEX_FILE).exists());
    assert_eq!(
        load_slang_index(&state).to_json(),
        host.engine.slang_index().to_json()
    );
    assert_eq!(
        load_meme_index(&state).to_json(),
        host.engine.meme_index().to_json()
    );
}

#[test]
fn unreadable_state_files_start_fresh() {
    let dir = tempfile::TempDir::new().unwrap();
    for file in [INDEX_FILE, MEME_INDEX_FILE, REMIXED_FILE] {
        std::fs::write(dir.path().join(file), "not json").unwrap();
    }
    assert_eq!(load_slang_index(dir.path()).len("IN"), 0);
    assert_eq!(load_meme_index(dir.path()).len("IN"), 0);
    assert!(load_remixed(dir.path()).is_empty());
    // Missing files are a fresh start too.
    let empty = tempfile::TempDir::new().unwrap();
    assert_eq!(load_slang_index(empty.path()).len("IN"), 0);
    assert_eq!(load_meme_index(empty.path()).len("IN"), 0);
}

#[test]
fn the_meme_cooldown_follows_its_env_override() {
    if std::env::var(MEME_COOLDOWN_ENV).is_err() {
        assert_eq!(
            rating_policy().meme_cooldown_turns,
            RatingPolicy::default().meme_cooldown_turns
        );
    }
}

#[test]
fn a_failed_write_leaves_no_temp_file() {
    let dir = tempfile::TempDir::new().unwrap();
    // Renaming over a directory fails; the temp file must be cleaned up.
    let target = dir.path().join("taken");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("child"), "x").unwrap();
    assert!(write_atomic(&target, "{}").is_err());
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty());
}
