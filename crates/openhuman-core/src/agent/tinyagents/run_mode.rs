//! Per-thread [`RunMode`] registry (Plan vs Build).
//!
//! `tinyagents_harness::middleware::{RunMode, RunModeHandle, plan_mode_middleware}`
//! (vendor tinyagents#211) gate side-effecting tools per-run via a live
//! [`RunModeHandle`] the host can flip without restarting the run. OpenHuman's
//! unit of "a run" for this purpose is a chat *thread*: the same thread is
//! driven through many independent turns (one `assemble_turn_harness` call
//! each), so the mode has to live somewhere that outlives any one turn's
//! [`crate::agent::tinyagents::host::OpenHumanRunContext`] — this process-wide,
//! thread_id-keyed registry is that home.
//!
//! `plan_exit` (the tool) and the `agent.set_run_mode` / `agent.get_run_mode`
//! RPCs both read/write through here; `turn_runner` looks the handle up by
//! `OpenHumanRunContext::thread_id` right before assembling each turn's
//! harness and pushes `plan_mode_middleware(handle, ..)` when a handle exists
//! for the thread.

use std::collections::HashMap;
use std::sync::OnceLock;

use parking_lot::Mutex;
use tinyagents_harness::middleware::{RunMode, RunModeHandle};

use crate::core::bus::BUS;
use crate::core::events::DomainEvent;

fn registry() -> &'static Mutex<HashMap<String, RunModeHandle>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, RunModeHandle>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Returns the [`RunModeHandle`] for `thread_id`, creating one (starting in
/// [`RunMode::Build`]) if none exists yet. Cloning a `RunModeHandle` shares
/// the same underlying atomic, so every clone (a live turn's middleware, this
/// registry's own copy, a later RPC call) observes the same live value.
pub fn handle_for_thread(thread_id: &str) -> RunModeHandle {
    let mut map = registry().lock();
    map.entry(thread_id.to_string())
        .or_insert_with(|| RunModeHandle::new(RunMode::Build))
        .clone()
}

/// Returns the current mode for `thread_id` without creating a handle —
/// `Build` (the default) when the thread has never toggled plan mode.
pub fn get_mode(thread_id: &str) -> RunMode {
    registry()
        .lock()
        .get(thread_id)
        .map(|h| h.get())
        .unwrap_or_default()
}

/// Sets the mode for `thread_id` (creating a handle if needed) and publishes
/// `DomainEvent::ThreadRunModeChanged` so the web channel can bridge a
/// `run_mode_changed` socket event. A no-op publish-wise when the mode is
/// already what was requested — still safe to call unconditionally.
pub fn set_mode(thread_id: &str, mode: RunMode) {
    let handle = handle_for_thread(thread_id);
    let changed = handle.get() != mode;
    handle.set(mode);
    if changed {
        tracing::info!(
            thread_id = %thread_id,
            mode = mode_label(mode),
            "[agent::run_mode] thread run mode changed"
        );
        BUS.publish(DomainEvent::ThreadRunModeChanged {
            thread_id: thread_id.to_string(),
            mode: mode_label(mode).to_string(),
        });
    }
}

/// Stable wire label for a [`RunMode`] — `"plan"` / `"build"`.
pub fn mode_label(mode: RunMode) -> &'static str {
    match mode {
        RunMode::Plan => "plan",
        RunMode::Build => "build",
    }
}

/// Parses a wire label back into a [`RunMode`]. Unrecognized input maps to
/// `None` so callers can reject it rather than silently defaulting.
pub fn parse_mode_label(label: &str) -> Option<RunMode> {
    match label {
        "plan" => Some(RunMode::Plan),
        "build" => Some(RunMode::Build),
        _ => None,
    }
}

#[cfg(test)]
#[path = "run_mode_tests.rs"]
mod tests;
