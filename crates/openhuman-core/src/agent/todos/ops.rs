//! OpenHuman host adapter over TinyAgents' `todos`.
//!
//! A todo list is scoped to one agent session ([`TodoScope::Session`]) or,
//! when a tool runs with no session at all, to a scratch list
//! ([`TodoScope::Scratch`]). Both live in the workspace-scoped [`store`];
//! validation, the whole-list write and rendering are TinyAgents'. This file
//! only maps a scope onto a store key. `clear` is for tests and cleanup.

use std::path::Path;
use std::sync::Arc;

use tinyagents_graph::todos::store as todos;
use tinyagents_harness::store::Store;

use crate::agent::tinyagents::todos::{session_todos_store, SCRATCH_SESSION_ID};
pub use crate::agent::todos::types::{TodoItem, TodoStatus};
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

/// The workspace-scoped store every session's list lives in.
pub fn store(workspace_dir: &Path) -> Arc<dyn Store> {
    session_todos_store(workspace_dir)
}

pub async fn replace(
    workspace_dir: &Path,
    scope: &TodoScope,
    items: Vec<TodoItem>,
) -> Result<TodosSnapshot, String> {
    todos::replace(&store(workspace_dir), scope.key(), items)
        .await
        .map_err(|error| error.to_string())
}

pub async fn clear(workspace_dir: &Path, scope: &TodoScope) -> Result<TodosSnapshot, String> {
    todos::clear(&store(workspace_dir), scope.key())
        .await
        .map_err(|error| error.to_string())
}

pub async fn list(workspace_dir: &Path, scope: &TodoScope) -> Result<TodosSnapshot, String> {
    todos::list(&store(workspace_dir), scope.key())
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
