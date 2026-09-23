//! The in-process store behind the session todo list.
//!
//! Todos are session state, the way Claude Code and Codex keep them: one list
//! per agent session, alive for the life of the process, gone on restart. The
//! transcript still records every list the model wrote, and the frontend
//! renders the latest one from the turn's `todo` tool call.

use std::sync::{Arc, OnceLock};

use tinyagents_harness::store::{InMemoryStore, Store};

/// The process-wide store every session's list lives in, keyed by session id.
pub fn session_todos_store() -> Arc<dyn Store> {
    static STORE: OnceLock<Arc<dyn Store>> = OnceLock::new();
    STORE.get_or_init(|| Arc::new(InMemoryStore::new())).clone()
}

/// Synthetic key for a tool call that has no session at all (a bare
/// `Tool::execute` in a test).
pub const SCRATCH_SESSION_ID: &str = "_scratch_";
