//! Behavior tests for `threads::ops::live_state` — direct store-level checks
//! that don't need the process-wide `Config::load_or_init()` a `workspace_dir()`
//! RPC call resolves against (full RPC-path coverage is in
//! `tests/json_rpc_e2e.rs`).

use super::*;
use crate::agent::goals::goal_to_value;
use crate::agent::todos::ops::{TodoItem, TodoStatus};

#[test]
fn thread_live_state_request_parses_thread_id() {
    let parsed: ThreadLiveStateRequest =
        serde_json::from_value(serde_json::json!({ "thread_id": "thread-1" })).unwrap();
    assert_eq!(parsed.thread_id, "thread-1");
}

/// `goal_to_value` (the field `goal_get`'s response and `ThreadGoalUpdated`
/// share) round-trips a goal's shape losslessly — a smoke check that the
/// shared serializer doesn't silently drop fields the frontend goal chip
/// reads.
#[tokio::test]
async fn goal_to_value_round_trips_the_stored_goal() {
    let dir = std::env::temp_dir().join(format!(
        "openhuman-goal-live-state-test-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let goal = crate::agent::goals::store::set(&dir, "thread-goal-live", "ship it", Some(1000))
        .await
        .unwrap();
    let value = goal_to_value(&goal);
    assert_eq!(value["objective"], "ship it");
    assert_eq!(value["status"], "active");
    assert_eq!(value["tokenBudget"], 1000);
}

/// `todos_get`'s store read returns the items a `TodoTool` call wrote under
/// the same thread-id key.
#[tokio::test]
async fn todos_get_reads_back_what_the_todo_tool_wrote() {
    let dir = std::env::temp_dir().join(format!(
        "openhuman-todos-live-state-test-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let scope = crate::agent::todos::ops::TodoScope::Session {
        id: "thread-todos-live".to_string(),
    };
    crate::agent::todos::ops::replace(
        &dir,
        &scope,
        vec![TodoItem::with_status("write tests", TodoStatus::InProgress)],
    )
    .await
    .unwrap();

    let response = todos_get(ThreadLiveStateRequest {
        thread_id: "thread-todos-live".to_string(),
    })
    .await
    .unwrap();
    let json = response.into_cli_compatible_json().unwrap();
    let todos = json["result"]["todos"].as_array().unwrap();
    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0]["content"], "write tests");
}
