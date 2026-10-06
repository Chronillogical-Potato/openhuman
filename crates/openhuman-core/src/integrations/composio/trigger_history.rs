//! Process-global handle to the persistent Composio trigger archive.
//!
//! The archive itself (daily JSONL under `<workspace>/state/triggers/`, exclusive
//! file locking on append, newest-first reads that skip corrupt lines) is
//! `tinyconnectors::triggers::TriggerArchive`. What stays here is the
//! host lifecycle: one archive per process, opened once at startup and read by
//! the trigger subscriber.

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use tinyconnectors::triggers::TriggerArchive;

static GLOBAL_TRIGGER_HISTORY: OnceLock<Arc<TriggerArchive>> = OnceLock::new();

/// Open the archive under `<workspace_dir>/state/triggers` and install it as the
/// process-global store. Idempotent for the same workspace; a different
/// workspace after initialization is an error.
pub fn init_global(workspace_dir: PathBuf) -> Result<(), String> {
    let state_dir = workspace_dir.join("state");
    let archive = Arc::new(
        TriggerArchive::open(&state_dir).map_err(|error| format!("[composio][history] {error}"))?,
    );
    let installed = GLOBAL_TRIGGER_HISTORY.get_or_init(|| archive.clone());
    if installed.archive_dir() == archive.archive_dir() {
        tracing::debug!(
            archive_dir = %archive.archive_dir().display(),
            "[composio][history] archive initialized"
        );
        return Ok(());
    }
    Err(format!(
        "[composio][history] global store already initialized for {} while attempting {}",
        installed.archive_dir().display(),
        archive.archive_dir().display()
    ))
}

/// The process-global archive, if [`init_global`] has run.
pub fn global() -> Option<Arc<TriggerArchive>> {
    GLOBAL_TRIGGER_HISTORY.get().cloned()
}
