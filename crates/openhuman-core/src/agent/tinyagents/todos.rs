//! OpenHuman integration for the TinyAgents task-board implementation.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use tinyagents_harness::store::{InMemoryStore, Store};

use crate::agent::session_import::ops::open_session_stores;

/// Open the durable TinyAgents store used by per-thread task boards.
pub fn todos_store(workspace_dir: &Path) -> Arc<dyn Store> {
    Arc::new(open_session_stores(workspace_dir).kv)
}

/// Shared ephemeral TinyAgents store used when a tool has no thread context.
pub fn scratch_todos_store() -> Arc<dyn Store> {
    static STORE: OnceLock<Arc<dyn Store>> = OnceLock::new();
    STORE.get_or_init(|| Arc::new(InMemoryStore::new())).clone()
}

/// Synthetic thread key for the process-global scratch board.
pub const SCRATCH_THREAD_ID: &str = "_scratch_";
