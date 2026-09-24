//! The persisted store behind the session todo list.
//!
//! Todos are session state, the way Claude Code and Codex keep them: one list
//! per conversation thread. Persisted under
//! `{workspace}/tinyagents_store/kv/` — the same `FileStore` tree
//! `open_session_stores` opens for thread goals (`agent::goals::store`) — so a
//! list survives a core restart instead of resetting with the old
//! process-global `InMemoryStore`. The transcript still records every list
//! the model wrote, and the frontend renders the latest one from the turn's
//! `todo` tool call.

use std::path::Path;
use std::sync::Arc;

use tinyagents_harness::store::Store;

use crate::agent::session_import::ops::open_session_stores;

/// The `workspace`-scoped store every session's list lives in, keyed by
/// session/thread id. Opened fresh per call (cheap — `FileStore` just holds a
/// root path) so it always reflects the caller's current workspace.
pub fn session_todos_store(workspace_dir: &Path) -> Arc<dyn Store> {
    Arc::new(open_session_stores(workspace_dir).kv)
}

/// Synthetic key for a tool call that has no session at all (a bare
/// `Tool::execute` in a test).
pub const SCRATCH_SESSION_ID: &str = "_scratch_";
