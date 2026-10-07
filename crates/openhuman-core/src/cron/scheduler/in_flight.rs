//! Which cron jobs are running right now.
//!
//! The scheduler dispatches each due job onto its own task and returns to its
//! poll loop, so the old "the loop is busy until the batch finishes" guarantee
//! no longer keeps a job from being picked up twice. This registry does: a job
//! is in flight from dispatch until its run is persisted. It counts rather than
//! flags because a manual "Run now" may legitimately overlap a scheduled run of
//! a job that is not single-flight, and the first to finish must not mark the
//! other one done.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, PoisonError};

static RUNNING: LazyLock<Mutex<HashMap<String, usize>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Held for the duration of one run; releases it on drop, including on panic
/// or cancellation.
#[must_use = "the job counts as running only while the guard is held"]
pub(crate) struct InFlightGuard {
    job_id: String,
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        let mut running = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(count) = running.get_mut(&self.job_id) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                running.remove(&self.job_id);
            }
        }
        tracing::trace!(job_id = %self.job_id, "[cron:in_flight] run released");
    }
}

/// Mark `job_id` as running until the returned guard drops.
pub(crate) fn enter(job_id: &str) -> InFlightGuard {
    *RUNNING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .entry(job_id.to_string())
        .or_insert(0) += 1;
    tracing::trace!(job_id, "[cron:in_flight] run entered");
    InFlightGuard {
        job_id: job_id.to_string(),
    }
}

/// Whether any run of `job_id` is in progress.
pub(crate) fn is_running(job_id: &str) -> bool {
    RUNNING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .contains_key(job_id)
}

#[cfg(test)]
#[path = "in_flight_tests.rs"]
mod tests;
