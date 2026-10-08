//! Shared in-memory state for the web-channel turn dispatcher: the per-thread
//! session cache, the in-flight turn tables, and the small helpers that key
//! and mutate them.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio::sync::Mutex;

use super::super::types::{InFlightEntry, ParallelEntry, SessionEntry};

type ThreadSessions = Mutex<HashMap<String, SessionEntry>>;
type InFlight = Mutex<HashMap<String, InFlightEntry>>;
type ParallelInFlight = Mutex<HashMap<String, ParallelEntry>>;

/// The current agent context's per-thread session cache.
pub(crate) fn thread_sessions() -> Arc<ThreadSessions> {
    crate::core::runtime::current_slot::<ThreadSessions>()
}

/// The current agent context's primary in-flight turns, keyed by thread.
pub(crate) fn in_flight() -> Arc<InFlight> {
    crate::core::runtime::current_slot::<InFlight>()
}

/// Parallel (forked) turns, keyed by `request_id`. A separate lane from
/// [`in_flight`] (which holds one primary, interrupt-able turn per thread) so
/// any number of concurrent `QueueMode::Parallel` turns can run on the same
/// thread without touching interrupt/steer/queue semantics.
pub(crate) fn parallel_in_flight() -> Arc<ParallelInFlight> {
    crate::core::runtime::current_slot::<ParallelInFlight>()
}

pub(crate) fn key_for(thread_id: &str) -> String {
    thread_id.to_string()
}

pub(crate) fn event_session_id_for(client_id: &str, thread_id: &str) -> String {
    json!({
        "client_id": client_id,
        "thread_id": thread_id,
    })
    .to_string()
}

/// Cooperatively cancel an in-flight turn, with a hard `abort()` backstop.
///
/// Cancelling the token makes the turn's `tokio::select!` arm fire, dropping
/// the turn future at its next await point (cancelling the in-flight LLM
/// request and releasing locks cleanly). The detached backstop hard-aborts the
/// task only if it has not finished unwinding within a short grace period, so a
/// wedged turn can never leak. Returns the cancelled turn's request id.
pub(crate) fn cancel_in_flight_gracefully(entry: InFlightEntry) -> String {
    let request_id = entry.request_id.clone();
    entry.cancel_token.cancel();
    let mut handle = entry.handle;
    tokio::spawn(async move {
        tokio::select! {
            _ = &mut handle => {}
            _ = tokio::time::sleep(Duration::from_secs(5)) => {
                log::warn!(
                    "[web-channel] cooperative cancel did not finish within grace period — hard-aborting backstop"
                );
                handle.abort();
            }
        }
    });
    request_id
}

pub async fn invalidate_thread_sessions(thread_id: &str) {
    let mut sessions = thread_sessions().lock_owned().await;
    let keys_to_remove: Vec<String> = sessions
        .keys()
        .filter(|k| k.as_str() == thread_id || k.ends_with(&format!("::{thread_id}")))
        .cloned()
        .collect();
    for key in &keys_to_remove {
        sessions.remove(key);
    }
    if !keys_to_remove.is_empty() {
        log::debug!(
            "[web-channel] invalidated {} cached session(s) for thread_id={}",
            keys_to_remove.len(),
            thread_id
        );
    }
}

pub async fn in_flight_entries_for_test() -> Vec<(String, String)> {
    let guard = in_flight().lock_owned().await;
    guard
        .iter()
        .map(|(k, v)| (k.clone(), v.request_id.clone()))
        .collect()
}

/// Test-only host-routing seam: drain one lane from the active turn's real
/// TinyAgents queue so acceptance tests can assert that web metadata survives
/// queue admission. Production code only observes this queue through its
/// status and terminal dispatch paths.
#[cfg(test)]
pub async fn drain_queued_turns_for_test(
    thread_id: &str,
    lane: tinyagents_harness::run_queue::QueueLane,
) -> Vec<crate::agent::queued_turn::QueuedTurn> {
    let guard = in_flight().lock_owned().await;
    match guard.get(&key_for(thread_id)) {
        Some(entry) => entry.run_queue.drain(lane).await,
        None => Vec::new(),
    }
}

/// Test accessor: `(request_id, thread_id)` for every in-flight parallel turn.
#[cfg(any(test, debug_assertions))]
pub async fn parallel_in_flight_entries_for_test() -> Vec<(String, String)> {
    let guard = parallel_in_flight().lock_owned().await;
    guard
        .iter()
        .map(|(request_id, entry)| (request_id.clone(), entry.thread_id.clone()))
        .collect()
}

/// Whether a cancel request should tear down the turn currently in flight for a
/// thread.
///
/// `requested` is the `request_id` the caller is cancelling; `None` means an
/// unscoped stop ("cancel whatever is running", e.g. a Stop button or a session
/// teardown). `in_flight` is the `request_id` currently registered for the
/// thread.
///
/// A *scoped* cancel matches only its own request. This is the fix for #4760: a
/// client that times out on request A and then sends request B — which
/// supersedes A on the same thread — must not have A's late-arriving cancel tear
/// down B. Scoping the cancel to A makes it a no-op once B is in flight, so the
/// newer turn survives instead of being killed at t=0.
pub fn cancel_should_target(requested: Option<&str>, in_flight: &str) -> bool {
    match requested {
        Some(rid) => rid == in_flight,
        None => true,
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
