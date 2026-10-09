//! Shared in-memory state for the web-channel turn dispatcher: the per-thread
//! session cache, the in-flight turn tables, and the small helpers that key
//! and mutate them.

use std::collections::HashMap;
use std::time::Duration;

use once_cell::sync::Lazy;
use serde_json::json;
use tokio::sync::Mutex;

use super::super::types::{InFlightEntry, ParallelEntry, SessionEntry};

pub(crate) static THREAD_SESSIONS: Lazy<Mutex<HashMap<String, SessionEntry>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub(crate) static IN_FLIGHT: Lazy<Mutex<HashMap<String, InFlightEntry>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Parallel (forked) turns, keyed by `request_id`. A separate lane from
/// `IN_FLIGHT` (which holds one primary, interrupt-able turn per thread) so any
/// number of concurrent `QueueMode::Parallel` turns can run on the same thread
/// without touching interrupt/steer/queue semantics. See `QueueMode::Parallel`.
pub(crate) static PARALLEL_IN_FLIGHT: Lazy<Mutex<HashMap<String, ParallelEntry>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// The map key for `thread_id` in the calling scope.
///
/// Thread ids are chosen by callers and are only unique per agent, so a turn
/// running for an embedded agent ([`CoreContext::session_agent`]) is keyed
/// `<agent>::<thread>`. Two agents that pick the same thread id then get two
/// cache entries and two in-flight slots instead of sharing one live session.
/// Outside an agent scope the key stays the bare thread id.
///
/// [`CoreContext::session_agent`]: crate::core::runtime::CoreContext::session_agent
pub(crate) fn key_for(thread_id: &str) -> String {
    let agent = crate::core::runtime::CoreContext::current()
        .and_then(|context| context.session_agent().map(str::to_owned));
    scoped_key(agent.as_deref(), thread_id)
}

/// Injective encoding of `(session_agent, thread_id)`. Both parts are caller
/// controlled, so a plain `agent::thread` join would let `("a", "b::c")` and
/// `("a::b", "c")` (or an unscoped `"a::b"`) share one slot. A scoped key is
/// `\x1f<agent byte length>:<agent><thread>`; an unscoped key is the bare
/// thread id, except that an id starting with `\x1f` gets a second `\x1f`
/// prepended so it can never look like a scoped key.
pub(crate) fn scoped_key(session_agent: Option<&str>, thread_id: &str) -> String {
    match session_agent {
        Some(agent) => format!("\u{1f}{}:{agent}{thread_id}", agent.len()),
        None if thread_id.starts_with('\u{1f}') => format!("\u{1f}{thread_id}"),
        None => thread_id.to_string(),
    }
}

/// The thread id a [`scoped_key`] was built from, ignoring its agent scope.
fn thread_id_of_key(key: &str) -> Option<&str> {
    let Some(rest) = key.strip_prefix('\u{1f}') else {
        return Some(key);
    };
    if rest.starts_with('\u{1f}') {
        return Some(rest);
    }
    let (len, tail) = rest.split_once(':')?;
    let len: usize = len.parse().ok()?;
    tail.get(len..)
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
    let mut sessions = THREAD_SESSIONS.lock().await;
    let keys_to_remove: Vec<String> = sessions
        .keys()
        .filter(|k| thread_id_of_key(k) == Some(thread_id))
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
    let guard = IN_FLIGHT.lock().await;
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
    let guard = IN_FLIGHT.lock().await;
    match guard.get(&key_for(thread_id)) {
        Some(entry) => entry.run_queue.drain(lane).await,
        None => Vec::new(),
    }
}

/// Test accessor: `(request_id, thread_id)` for every in-flight parallel turn.
#[cfg(any(test, debug_assertions))]
pub async fn parallel_in_flight_entries_for_test() -> Vec<(String, String)> {
    let guard = PARALLEL_IN_FLIGHT.lock().await;
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
