//! In-process handlers for system cron jobs. Stub (RED).

use std::sync::Arc;

use futures::future::BoxFuture;

/// The job a handler is asked to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemJobContext {
    /// The cron row's id.
    pub job_id: String,
    /// The system job's name (`system:<name>`).
    pub name: String,
}

/// A handler for one system job.
pub type SystemJobHandler =
    Arc<dyn Fn(SystemJobContext) -> BoxFuture<'static, Result<(), String>> + Send + Sync>;

/// Keeps a handler registered while held.
pub struct SystemJobRegistration;

/// Register `handler` for system job `name`.
pub fn register(_name: &str, _handler: SystemJobHandler) -> SystemJobRegistration {
    SystemJobRegistration
}

/// Whether `name` has a handler.
pub fn has_handler(_name: &str) -> bool {
    false
}

/// Run the handler for `ctx.name`.
pub async fn dispatch(_ctx: SystemJobContext) -> Option<Result<(), String>> {
    None
}

#[cfg(test)]
#[path = "system_job_handlers_tests.rs"]
mod tests;
