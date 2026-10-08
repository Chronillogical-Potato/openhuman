//! Memory's event subscriber.
//!
//! `memory::system_jobs` runs memory's cron jobs
//! ([`DomainEvent::CronSystemJobDue`]):
//!
//! - `memory_sources_sync` starts the due source syncs;
//! - `memory_background` runs the queued belief builds and deferred ingests
//!   (`lifecycle::jobs`), after resuming a v1 import the app quit in the
//!   middle of (`import::resume_interrupted`).
//!
//! Turns are not ingested from the bus: the session host calls the lifecycle
//! hooks itself, under the session's own config (`lifecycle::hooks`).

use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use chrono::Utc;
use tinybus::{EventHandler, SubscriptionHandle};

use crate::core::events::DomainEvent;

use super::lifecycle::jobs::BACKGROUND_JOB;

/// Cron job that starts due source syncs.
pub const SOURCES_SYNC_JOB: &str = "memory_sources_sync";

/// The retired `context.md` refresh job; `cron::system_jobs` removes its row.
pub const RETIRED_CONTEXT_REFRESH_JOB: &str = "memory_context_refresh";

static JOBS_HANDLE: OnceLock<SubscriptionHandle> = OnceLock::new();

struct SystemJobsSubscriber;

#[async_trait]
impl EventHandler<DomainEvent> for SystemJobsSubscriber {
    fn name(&self) -> &str {
        "memory::system_jobs"
    }

    fn domains(&self) -> Option<&[&str]> {
        Some(&["cron"])
    }

    async fn handle(&self, event: &DomainEvent) {
        let DomainEvent::CronSystemJobDue { job } = event else {
            return;
        };
        if job != SOURCES_SYNC_JOB && job != BACKGROUND_JOB {
            return;
        }
        let config = match crate::config::rpc::load_config_with_timeout().await {
            Ok(config) => config,
            Err(error) => {
                tracing::debug!(error = %error, job = %job, "[memory:bus] config unavailable");
                return;
            }
        };
        run_system_job(&config, job).await;
    }
}

/// Runs one memory cron job against `config`.
pub async fn run_system_job(config: &crate::config::Config, job: &str) {
    match job {
        SOURCES_SYNC_JOB => {
            let started = super::sources::sync_due(config, Utc::now());
            tracing::debug!(
                started = started.len(),
                "[memory:bus] due source syncs started"
            );
        }
        BACKGROUND_JOB => {
            super::import::resume_interrupted(config).await;
            super::layout_migration::tick(
                config,
                std::sync::Arc::new(super::layout_migration::AppHost),
                std::sync::Arc::new(super::import::scheduler_paused),
            );
            super::lifecycle::jobs::run_due(config).await;
        }
        _ => {}
    }
}

/// Registers memory's subscriber. Idempotent.
pub fn register_memory_subscribers() {
    if JOBS_HANDLE.get().is_none() {
        match crate::core::bus::BUS.subscribe(Arc::new(SystemJobsSubscriber)) {
            Some(handle) => {
                let _ = JOBS_HANDLE.set(handle);
                tracing::info!("[memory:bus] memory subscribers registered");
            }
            None => tracing::warn!("[memory:bus] system jobs not registered: no bus"),
        }
    }
}

#[cfg(test)]
#[path = "bus_tests.rs"]
mod tests;
