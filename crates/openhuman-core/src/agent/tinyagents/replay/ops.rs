//! Read-only business logic for the agent replay/status RPC surface
//! (workstream 05.x): a workspace-to-store adapter over the upstream readers in
//! `tinyagents_harness::observability::replay`. Every function opens the same
//! `{workspace}/tinyagents_store/{kv,journal}` stores the turn journal writes
//! and never writes, mutates, or bypasses any security/approval/sandbox gate.

use std::path::Path;

use tinyagents_harness::events::HarnessRunStatus;
use tinyagents_harness::observability::replay::{self, RunEventsPage};
use tinyagents_harness::observability::{FileStatusStore, StoreEventJournal};
use tinyagents_session::transcript::import::ops::open_session_stores;

/// Paged late-attach replay reader over the durable journal under `workspace`.
pub(crate) async fn read_run_events_page(
    workspace: &Path,
    run_id: &str,
    offset: u64,
    limit: u64,
) -> anyhow::Result<RunEventsPage> {
    let journal = StoreEventJournal::new(open_session_stores(workspace).journal);
    replay::read_run_events_page(&journal, run_id, offset, limit)
        .await
        .map_err(|e| {
            anyhow::anyhow!("[agent] replay read_run_events_page failed run_id={run_id}: {e}")
        })
}

/// Latest durable status for `run_id`, or `None` when the run is unknown.
pub(crate) async fn read_run_status(
    workspace: &Path,
    run_id: &str,
) -> anyhow::Result<Option<HarnessRunStatus>> {
    let store = FileStatusStore::new(open_session_stores(workspace).kv);
    replay::read_run_status(&store, run_id)
        .await
        .map_err(|e| anyhow::anyhow!("[agent] replay read_run_status failed run_id={run_id}: {e}"))
}

/// Active runs, optionally filtered by `thread_id` and/or `root_run_id`.
pub(crate) async fn list_active_runs(
    workspace: &Path,
    thread_id: Option<&str>,
    root_run_id: Option<&str>,
) -> anyhow::Result<Vec<HarnessRunStatus>> {
    let store = FileStatusStore::new(open_session_stores(workspace).kv);
    replay::list_active_runs(&store, thread_id, root_run_id)
        .await
        .map_err(|e| anyhow::anyhow!("[agent] replay list_active_runs failed: {e}"))
}
