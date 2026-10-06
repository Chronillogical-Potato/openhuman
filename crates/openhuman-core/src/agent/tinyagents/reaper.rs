//! Startup reconciliation for orphaned agent runs.
//!
//! The sweep itself (`tinyagents_harness::observability::reap_orphaned_runs`)
//! lives upstream; this host adapter only opens the workspace's durable status
//! store. It is the one *writer* over the status seam that
//! [`crate::agent::tinyagents::journal`] exposes; the replay/status controllers
//! ([`super::replay`]) stay strictly read-only.

use std::path::Path;

use tinyagents_harness::observability::FileStatusStore;
use tinyagents_session::transcript::import::ops::open_session_stores;

/// Reap every run left non-terminal by a previous process, returning the number
/// of runs moved to `Cancelled`. Best-effort; never blocks boot.
pub(crate) async fn reap_orphaned_runs(workspace: &Path) -> usize {
    log::debug!(
        "[agent] startup run sweep workspace={}",
        workspace.display()
    );
    // A host session store repairs its own runs (`SessionStoreProvider::recover`,
    // run at boot): its status records are per agent, not in this workspace.
    if crate::agent::session_store::is_installed() {
        log::debug!("[agent] startup run sweep skipped: the host session store recovers its runs");
        return 0;
    }
    let store = FileStatusStore::new(open_session_stores(workspace).kv);
    tinyagents_harness::observability::reap_orphaned_runs(&store).await
}

#[cfg(test)]
#[path = "reaper_tests.rs"]
mod tests;
