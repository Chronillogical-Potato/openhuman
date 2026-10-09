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
///
/// Skipped on a storage backend other processes may share (MongoDB): there a
/// non-terminal status can belong to a run another replica is still driving,
/// and cancelling it would hide that run from active and late-attach status.
///
/// With a storage backend every agent keeps its own status store, so the
/// sweep visits `local` and then each known agent (`crate::storage::agents`).
pub(crate) async fn reap_orphaned_runs(workspace: &Path) -> usize {
    let shared = crate::storage::installed_is_shared();
    crate::storage::agents::for_each_scope("run reaper", || reap_unless_shared(workspace, shared))
        .await
        .into_iter()
        .map(|(_, reaped)| reaped)
        .sum()
}

async fn reap_unless_shared(workspace: &Path, shared: bool) -> usize {
    if shared {
        log::info!("[agent] startup run sweep skipped: the storage backend is shared");
        return 0;
    }
    log::debug!(
        "[agent] startup run sweep workspace={}",
        workspace.display()
    );
    // With a host session store the status records live in its stores: sweep
    // the ones this process's own (default-agent) work wrote there.
    if let Some(stores) = crate::agent::session_store::current() {
        let store = FileStatusStore::over(stores.kv);
        return tinyagents_harness::observability::reap_orphaned_runs(&store).await;
    }
    let store = FileStatusStore::new(open_session_stores(workspace).kv);
    tinyagents_harness::observability::reap_orphaned_runs(&store).await
}

#[cfg(test)]
#[path = "reaper_tests.rs"]
mod tests;
