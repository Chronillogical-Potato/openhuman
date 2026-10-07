//! Per-job run policy: retry budget and single-flight.
//!
//! Stub (RED).

use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::Config;

/// How the scheduler runs one job.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobPolicy {
    /// Retries after a failed attempt. `None` keeps the default.
    pub retries: Option<u32>,
    /// Skip a run that comes due while the previous one is still running.
    pub single_flight: bool,
}

pub(crate) fn db_path(config: &Config) -> PathBuf {
    config.workspace_dir.join("cron").join("jobs.db")
}

/// The policy stored for `job_id`.
pub fn get_policy(_config: &Config, _job_id: &str) -> Result<JobPolicy> {
    Ok(JobPolicy::default())
}

/// Store `policy` for `job_id`.
pub fn set_policy(_config: &Config, _job_id: &str, _policy: JobPolicy) -> Result<()> {
    Ok(())
}

/// Retry budget for a job with `policy`.
pub fn effective_retries(_config: &Config, _policy: &JobPolicy) -> u32 {
    0
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
