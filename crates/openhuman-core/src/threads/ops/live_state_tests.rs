//! Behavior tests for `threads::ops::live_state`.
//!
//! `goal_get`/`todos_get` resolve their workspace through
//! `Config::load_or_init()` (the same process-global config every RPC
//! handler reads), so — like the other `OPENHUMAN_WORKSPACE`-dependent config
//! tests — these serialize on `crate::config::TEST_ENV_LOCK` and point that
//! env var at a fresh tempdir for the duration of the test.

use super::*;
use crate::agent::todos::ops::{TodoItem, TodoStatus};
use crate::config::TEST_ENV_LOCK;

#[test]
fn thread_live_state_request_parses_thread_id() {
    let parsed: ThreadLiveStateRequest =
        serde_json::from_value(serde_json::json!({ "thread_id": "thread-1" })).unwrap();
    assert_eq!(parsed.thread_id, "thread-1");
}

struct WorkspaceGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    _tmp: tempfile::TempDir,
}

impl WorkspaceGuard {
    fn new() -> Self {
        let lock = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().expect("tempdir");
        unsafe {
            std::env::set_var("OPENHUMAN_WORKSPACE", tmp.path());
        }
        Self {
            _lock: lock,
            _tmp: tmp,
        }
    }
}

impl Drop for WorkspaceGuard {
    fn drop(&mut self) {
        unsafe {
            std::env::remove_var("OPENHUMAN_WORKSPACE");
        }
    }
}

/// `threads.goal_get` returns `{ goal: null }` for a thread with no goal, and
/// the full goal payload — the same shape `ThreadGoalUpdated` carries — once
/// one is set.
#[tokio::test]
async fn goal_get_reads_back_a_stored_goal() {
    let _ws = WorkspaceGuard::new();

    let empty = goal_get(ThreadLiveStateRequest {
        thread_id: "thread-goal-live".to_string(),
    })
    .await
    .unwrap();
    let empty_json = empty.into_cli_compatible_json().unwrap();
    assert!(empty_json["result"]["goal"].is_null(), "{empty_json}");

    let dir = crate::config::Config::load_or_init()
        .await
        .unwrap()
        .workspace_dir;
    crate::agent::goals::store::set(&dir, "thread-goal-live", "ship it", Some(1000))
        .await
        .unwrap();

    let filled = goal_get(ThreadLiveStateRequest {
        thread_id: "thread-goal-live".to_string(),
    })
    .await
    .unwrap();
    let filled_json = filled.into_cli_compatible_json().unwrap();
    let goal = &filled_json["result"]["goal"];
    assert_eq!(goal["objective"], "ship it");
    assert_eq!(goal["status"], "active");
}

/// `threads.todos_get` reads back what a `TodoTool` call (thread-id-keyed)
/// wrote for the same thread.
#[tokio::test]
async fn todos_get_reads_back_what_the_todo_tool_wrote() {
    let _ws = WorkspaceGuard::new();
    let dir = crate::config::Config::load_or_init()
        .await
        .unwrap()
        .workspace_dir;

    let empty = todos_get(ThreadLiveStateRequest {
        thread_id: "thread-todos-live".to_string(),
    })
    .await
    .unwrap();
    let empty_json = empty.into_cli_compatible_json().unwrap();
    assert!(empty_json["result"]["todos"].as_array().unwrap().is_empty());

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

    let filled = todos_get(ThreadLiveStateRequest {
        thread_id: "thread-todos-live".to_string(),
    })
    .await
    .unwrap();
    let filled_json = filled.into_cli_compatible_json().unwrap();
    let todos = filled_json["result"]["todos"].as_array().unwrap();
    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0]["content"], "write tests");
}
