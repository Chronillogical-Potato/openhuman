//! OpenHuman host adapter over [`tinyagents_graph::todos`].
//!
//! A todo list is scoped to one agent session ([`TodoScope::Session`]) or,
//! when a tool runs with no session at all, to a scratch list
//! ([`TodoScope::Scratch`]). Both live in the one in-process store; the
//! normalisation and rendering are TinyAgents'. The whole-list `replace` is
//! the only write the `todo` tool needs; `clear` is for tests and cleanup.

use serde::{Deserialize, Serialize};
use tinyagents_graph::todos::store as todos;

use crate::agent::tinyagents::todos::{session_todos_store, SCRATCH_SESSION_ID};
use crate::agent::todos::types::normalize_cards_for_wire;
pub use crate::agent::todos::types::{TaskBoardCard, TaskCardStatus};

pub use tinyagents_graph::todos::{parse_status, render_markdown};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodosSnapshot {
    pub session_id: Option<String>,
    pub cards: Vec<TaskBoardCard>,
    pub markdown: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TodoScope {
    Session { id: String },
    Scratch,
}

impl TodoScope {
    pub fn session_id(&self) -> Option<&str> {
        match self {
            Self::Session { id } => Some(id),
            Self::Scratch => None,
        }
    }

    fn key(&self) -> &str {
        self.session_id().unwrap_or(SCRATCH_SESSION_ID)
    }
}

fn snapshot(scope: &TodoScope, value: tinyagents_graph::todos::TodosSnapshot) -> TodosSnapshot {
    TodosSnapshot {
        session_id: scope.session_id().map(str::to_owned),
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

pub async fn replace(
    scope: &TodoScope,
    cards: Vec<TaskBoardCard>,
) -> Result<TodosSnapshot, String> {
    let store = session_todos_store();
    finish(scope, todos::replace(&store, scope.key(), cards).await)
}

pub async fn clear(scope: &TodoScope) -> Result<TodosSnapshot, String> {
    let store = session_todos_store();
    finish(scope, todos::clear(&store, scope.key()).await)
}

pub async fn list(scope: &TodoScope) -> Result<TodosSnapshot, String> {
    let store = session_todos_store();
    todos::list(&store, scope.key())
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
