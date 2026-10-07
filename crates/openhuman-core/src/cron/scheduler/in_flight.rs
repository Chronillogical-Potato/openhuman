//! Which cron jobs are running right now. Stub (RED).

/// Held for the duration of one run.
pub(crate) struct InFlightGuard;

/// Mark `job_id` as running until the returned guard drops.
pub(crate) fn enter(_job_id: &str) -> InFlightGuard {
    InFlightGuard
}

/// Whether any run of `job_id` is in progress.
pub(crate) fn is_running(_job_id: &str) -> bool {
    false
}

#[cfg(test)]
#[path = "in_flight_tests.rs"]
mod tests;
