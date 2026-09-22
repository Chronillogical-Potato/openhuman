//! OpenHuman host adapter over [`tinyagents_graph::todos`].
//!
//! A todo list is scoped to one conversation thread ([`TodoScope::Thread`])
//! or, when a tool runs with no thread at all, to a process-global scratch
//! list ([`TodoScope::Scratch`]). The store, normalisation and rendering are
//! TinyAgents'; this file only picks the store for a scope and reshapes the
//! snapshot for OpenHuman callers. The whole-list `replace` is the only
//! write the `todo` tool needs; `clear` is for tests and cleanup.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tinyagents_graph::todos::store as todos;
use tinyagents_harness::store::Store;

use crate::agent::tinyagents::todos::{scratch_todos_store, todos_store, SCRATCH_THREAD_ID};
use crate::agent::todos::types::normalize_cards_for_wire;
pub use crate::agent::todos::types::{TaskBoardCard, TaskCardStatus};

pub use tinyagents_graph::todos::{parse_status, render_markdown};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodosSnapshot {
    pub thread_id: Option<String>,
    pub cards: Vec<TaskBoardCard>,
    pub markdown: String,
}

#[derive(Debug, Clone)]
pub enum TodoScope {
    Thread {
        workspace_dir: PathBuf,
        thread_id: String,
    },
    Scratch,
}

impl TodoScope {
    pub fn thread_id(&self) -> Option<&str> {
        match self {
            Self::Thread { thread_id, .. } => Some(thread_id),
            Self::Scratch => None,
        }
    }
}

fn target(scope: &TodoScope) -> (Arc<dyn Store>, &str) {
    match scope {
        TodoScope::Thread {
            workspace_dir,
            thread_id,
        } => (todos_store(workspace_dir), thread_id),
        TodoScope::Scratch => (scratch_todos_store(), SCRATCH_THREAD_ID),
    }
}

fn snapshot(scope: &TodoScope, value: tinyagents_graph::todos::TodosSnapshot) -> TodosSnapshot {
    TodosSnapshot {
        thread_id: scope.thread_id().map(str::to_owned),
        cards: value.cards,
        markdown: value.markdown,
    }
}

fn finish(
    scope: &TodoScope,
    result: tinyagents_harness::error::Result<tinyagents_graph::todos::TodosSnapshot>,
) -> Result<TodosSnapshot, String> {
    let mut value = result.map_err(|error| error.to_string())?;
    normalize_cards_for_wire(&mut value.cards);
    Ok(snapshot(scope, value))
}

pub async fn replace(scope: &TodoScope, cards: Vec<TaskBoardCard>) -> Result<TodosSnapshot, String> {
    let (store, thread_id) = target(scope);
    finish(scope, todos::replace(&store, thread_id, cards).await)
}

pub async fn clear(scope: &TodoScope) -> Result<TodosSnapshot, String> {
    let (store, thread_id) = target(scope);
    finish(scope, todos::clear(&store, thread_id).await)
}

pub async fn list(scope: &TodoScope) -> Result<TodosSnapshot, String> {
    let (store, thread_id) = target(scope);
    todos::list(&store, thread_id)
        .await
        .map(|value| snapshot(scope, value))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
pub(crate) fn scratch_test_lock() -> std::sync::MutexGuard<'static, ()> {
    use std::sync::{Mutex, OnceLock};
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
