use super::*;
use std::sync::Mutex;
use tinyagents_session::testkit::conformance::session_store_conformance;
use tinyagents_session::turn_state::{TurnLifecycle, TurnState};

#[tokio::test]
async fn the_desktop_layout_meets_the_session_store_contract() {
    let dir = tempfile::tempdir().unwrap();
    session_store_conformance(&SqliteSessionStores::at(dir.path())).await;
    // Everything landed in the classic layout.
    assert!(dir.path().join("session_raw").is_dir());
    assert!(dir.path().join("tinyagents_store").is_dir());
}

#[test]
fn the_workspace_follows_the_resolver() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let current = Arc::new(Mutex::new(first.path().to_path_buf()));
    let resolver = current.clone();
    let stores = SqliteSessionStores::resolving(move || resolver.lock().unwrap().clone());
    assert_eq!(stores.workspace_dir().as_deref(), Some(first.path()));
    *current.lock().unwrap() = second.path().to_path_buf();
    assert_eq!(stores.workspace_dir().as_deref(), Some(second.path()));
    assert_eq!(
        stores.destination_key(),
        Some(second.path().to_string_lossy().into_owned())
    );
    assert!(format!("{stores:?}").contains("SqliteSessionStores"));
}

#[test]
fn recovery_interrupts_turns_left_in_flight() {
    let dir = tempfile::tempdir().unwrap();
    let stores = SqliteSessionStores::at(dir.path());
    let agent = stores.for_agent("orchestrator");
    agent
        .turn_states
        .put(&TurnState::started("t", "r", 4, "2026-01-01T00:00:00Z"))
        .unwrap();
    stores.recover().unwrap();
    assert_eq!(
        agent
            .turn_states
            .get("t")
            .unwrap()
            .map(|turn| turn.lifecycle),
        Some(TurnLifecycle::Interrupted)
    );
}
