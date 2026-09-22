//! OpenHuman host adapter over TinyAgents' `todos::session_list`.
//!
//! A todo list is scoped to one agent session ([`TodoScope::Session`]) or,
//! when a tool runs with no session at all, to a scratch list
//! ([`TodoScope::Scratch`]). Both live in the one in-process [`store`];
//! validation, the whole-list write and rendering are TinyAgents'. This file
//! only maps a scope onto a store key. `clear` is for tests and cleanup.

use std::sync::Arc;

use tinyagents_graph::todos::session_list;
use tinyagents_harness::store::Store;

use crate::agent::tinyagents::todos::{session_todos_store, SCRATCH_SESSION_ID};
pub use crate::agent::todos::types::{TaskBoardCard, TaskCardStatus};
pub use tinyagents_graph::todos::TodosSnapshot;

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

    /// The store key this scope's list lives under.
    pub fn key(&self) -> &str {
        self.session_id().unwrap_or(SCRATCH_SESSION_ID)
    }
}

/// The process-wide store every session's list lives in.
pub fn store() -> Arc<dyn Store> {
    session_todos_store()
}

pub async fn replace(scope: &TodoScope, cards: Vec<TaskBoardCard>) -> Result<TodosSnapshot, String> {
    session_list::write(&store(), scope.key(), cards)
        .await
        .map_err(|error| error.to_string())
}

pub async fn clear(scope: &TodoScope) -> Result<TodosSnapshot, String> {
    replace(scope, Vec::new()).await
}

pub async fn list(scope: &TodoScope) -> Result<TodosSnapshot, String> {
    session_list::read(&store(), scope.key())
        .await
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
