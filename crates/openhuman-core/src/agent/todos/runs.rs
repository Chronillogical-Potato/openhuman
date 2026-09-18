//! OpenHuman host adapter over [`tinyagents_graph::todos::runs`].
//!
//! TinyAgents owns the durable task-run record, the heartbeat, the staleness
//! policy, and the reclaim sweep (card back to `todo`, or parked at `blocked`
//! once a card has burned through its reclaim budget). What stays here is
//! OpenHuman's own shape around it: [`BoardLocation`] addressing (including the
//! process-global scratch board), RFC 3339 timestamps on the wire, and the
//! `TaskRunReclaimed` domain event.
//!
//! Run records live in the crate KV store beside the board itself
//! (`graph.todos.runs`), so a board and its run log can no longer drift apart
//! across a restart.

use tinyagents_graph::todos::runs as crate_runs;

pub use tinyagents_graph::todos::runs::{
    ReclaimDetail, ReclaimResult, RunLimits, RunOutcome, TaskRun, DEFAULT_CLAIM_TTL_SECS,
    DEFAULT_HEARTBEAT_STALE_SECS, DEFAULT_MAX_RECLAIM_COUNT,
};

use crate::agent::todos::types::normalize_timestamp_for_wire;

use super::ops::{target, BoardLocation};

/// Cadence of the background heartbeat spawned alongside an autonomous run.
const HEARTBEAT_TICK: std::time::Duration = crate_runs::DEFAULT_HEARTBEAT_TICK;

fn map_err<T>(result: tinyagents_harness::error::Result<T>) -> Result<T, String> {
    result.map_err(|error| error.to_string())
}

/// Crate stamps are unix-epoch milliseconds; OpenHuman logs and transcripts use
/// RFC 3339, so translate on the way out.
fn for_wire(mut run: TaskRun) -> TaskRun {
    run.started_at = normalize_timestamp_for_wire(&run.started_at);
    run.last_heartbeat_at = normalize_timestamp_for_wire(&run.last_heartbeat_at);
    run.completed_at = run
        .completed_at
        .as_deref()
        .map(normalize_timestamp_for_wire);
    run
}

pub async fn create_run(
    location: &BoardLocation,
    run_id: &str,
    card_id: &str,
    claimed_by: &str,
) -> Result<TaskRun, String> {
    let (store, thread_id) = target(location);
    let run = map_err(
        crate_runs::create_run(&store, thread_id, Some(run_id), card_id, claimed_by).await,
    )?;
    Ok(for_wire(run))
}

pub async fn update_heartbeat(location: &BoardLocation, run_id: &str) -> Result<(), String> {
    let (store, thread_id) = target(location);
    map_err(crate_runs::update_heartbeat(&store, thread_id, run_id).await)
}

pub async fn complete_run(
    location: &BoardLocation,
    run_id: &str,
    outcome: RunOutcome,
    error: Option<String>,
    evidence: Vec<String>,
) -> Result<TaskRun, String> {
    let (store, thread_id) = target(location);
    let run = map_err(
        crate_runs::complete_run(&store, thread_id, run_id, outcome, error, evidence).await,
    )?;
    Ok(for_wire(run))
}

pub async fn list_runs(
    location: &BoardLocation,
    card_id: Option<&str>,
) -> Result<Vec<TaskRun>, String> {
    let (store, thread_id) = target(location);
    let runs = map_err(crate_runs::list_runs(&store, thread_id, card_id).await)?;
    Ok(runs.into_iter().map(for_wire).collect())
}

pub async fn get_run(location: &BoardLocation, run_id: &str) -> Result<Option<TaskRun>, String> {
    let (store, thread_id) = target(location);
    let run = map_err(crate_runs::get_run(&store, thread_id, run_id).await)?;
    Ok(run.map(for_wire))
}

pub async fn find_stale_runs(
    location: &BoardLocation,
    limits: &RunLimits,
) -> Result<Vec<(TaskRun, String)>, String> {
    let (store, thread_id) = target(location);
    let stale = map_err(crate_runs::find_stale_runs(&store, thread_id, limits).await)?;
    Ok(stale
        .into_iter()
        .map(|(run, reason)| (for_wire(run), reason))
        .collect())
}

/// Reclaim stale runs and publish a `TaskRunReclaimed` event per reclaimed
/// card for other runtime consumers.
pub async fn reclaim_stale(
    location: &BoardLocation,
    limits: &RunLimits,
) -> Result<ReclaimResult, String> {
    let (store, thread_id) = target(location);
    let result = map_err(crate_runs::reclaim_stale(&store, thread_id, limits).await)?;

    if let Some(thread_id) = location.thread_id() {
        for detail in &result.details {
            crate::core::bus::BUS.publish(crate::core::events::DomainEvent::TaskRunReclaimed {
                run_id: detail.run_id.clone(),
                card_id: detail.card_id.clone(),
                thread_id: thread_id.to_string(),
                reason: detail.reason.clone(),
            });
        }
    }
    Ok(result)
}

/// Tick the run's heartbeat in the background until it completes or `cancel`
/// fires. Board-location addressing is resolved once, here, so the crate task
/// carries only a store and a thread id.
pub fn spawn_heartbeat_task(
    location: BoardLocation,
    run_id: String,
    cancel: tokio::sync::watch::Receiver<bool>,
) {
    let (store, thread_id) = target(&location);
    crate_runs::spawn_heartbeat_task(store, thread_id.to_string(), run_id, cancel, HEARTBEAT_TICK);
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "runs_tests.rs"]
mod tests;
