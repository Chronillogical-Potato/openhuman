//! Persisted sub-agent shapes: `subagent_sessions.json` (durable session store)
//! and the pause checkpoint. Literal files from the release before the message
//! type migration (`tests/fixtures/session_compat/`) must keep loading, replay
//! to the same model messages, and re-serialize to the same JSON, so a binary
//! of either generation can read what the other wrote.
//!
//! Goldens are written only with `OH_REGEN_SESSION_COMPAT=1` and must come
//! from the pre-migration code.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::agent::message_convert::history_to_messages;
use crate::agent::orchestration::subagent_sessions::DurableSubagentSession;
use crate::agent::subagent_host::SubagentCheckpointData;

fn fixture(name: &str) -> PathBuf {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/session_compat"
    ))
    .join(name)
}

fn read(name: &str) -> String {
    std::fs::read_to_string(fixture(name)).unwrap_or_else(|error| panic!("{name}: {error}"))
}

fn check_golden(name: &str, got: &Value) {
    let path = fixture(name);
    if std::env::var("OH_REGEN_SESSION_COMPAT").as_deref() == Ok("1") {
        std::fs::write(&path, serde_json::to_string_pretty(got).unwrap() + "\n").unwrap();
    }
    let want: Value = serde_json::from_str(&read(name)).unwrap();
    assert_eq!(
        &want, got,
        "{name}: model view of persisted history drifted"
    );
}

#[test]
fn durable_session_store_loads_replays_and_rewrites_identically() {
    let raw = read("subagent_sessions.json");
    let sessions: Vec<DurableSubagentSession> = serde_json::from_str(&raw).expect("load store");
    let history = sessions[0]
        .latest_history
        .as_ref()
        .expect("latest history persisted");
    check_golden(
        "subagent_sessions.golden.json",
        &serde_json::to_value(history_to_messages(history)).unwrap(),
    );
    // The store writes pretty JSON; an unchanged load/save cycle is bytewise
    // stable, which is what lets an older binary keep reading a newer file.
    assert_eq!(
        serde_json::to_string_pretty(&sessions).unwrap(),
        raw.trim_end()
    );
}

#[test]
fn pause_checkpoint_loads_replays_and_rewrites_identically() {
    let raw = read("subagent_checkpoint.json");
    let checkpoint: SubagentCheckpointData = serde_json::from_str(&raw).expect("load checkpoint");
    check_golden(
        "subagent_checkpoint.golden.json",
        &serde_json::to_value(history_to_messages(&checkpoint.history)).unwrap(),
    );
    // The legacy `toolkit_override` key is dropped on rewrite; everything else
    // is stable.
    let mut want: Value = serde_json::from_str(&raw).unwrap();
    want.as_object_mut().unwrap().remove("toolkit_override");
    assert_eq!(serde_json::to_value(&checkpoint).unwrap(), want);
}
