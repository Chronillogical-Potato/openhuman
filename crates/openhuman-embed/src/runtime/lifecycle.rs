//! Removing an agent from a running [`Runtime`].

use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::time::Duration;

use super::Runtime;
use crate::agent::AgentError;

/// How long [`Runtime::remove_agent`] waits for in-flight turns to unwind.
const REMOVE_IDLE_WAIT: Duration = Duration::from_secs(10);

/// A pending [`Runtime::remove_agent`]. Await it, optionally after
/// [`purge`](Self::purge).
#[must_use = "an agent is removed only when this is awaited"]
pub struct RemoveAgent<'a> {
    runtime: &'a Runtime,
    id: String,
    purge: bool,
}

impl RemoveAgent<'_> {
    /// Also delete the agent's home, `<workspace>/agents/<id>/`, with its
    /// transcripts, skills, job store and MCP store. Kept by default.
    pub fn purge(mut self) -> Self {
        self.purge = true;
        self
    }
}

impl<'a> IntoFuture for RemoveAgent<'a> {
    type Output = Result<(), AgentError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { self.runtime.remove(&self.id, self.purge).await })
    }
}

impl Runtime {
    /// Remove agent `id` from this runtime.
    ///
    /// In order: new turns on any handle to it are refused with
    /// [`CoreError::AgentRemoved`](crate::CoreError::AgentRemoved); turns in
    /// flight end with the same error, waited on for up to ten seconds; its
    /// parked approvals are denied with resolution `agent_removed`; its state
    /// slots and MCP host are dropped and its context deregistered, so its
    /// cron jobs stay dormant. The id is free for reuse once this returns.
    ///
    /// Cancellation is cooperative: a tool already executing when the agent
    /// is removed, or a sub-agent it detached, may finish after this returns.
    pub fn remove_agent(&self, id: &str) -> RemoveAgent<'_> {
        RemoveAgent {
            runtime: self,
            id: id.to_string(),
            purge: false,
        }
    }

    async fn remove(&self, id: &str, purge: bool) -> Result<(), AgentError> {
        let inner = {
            let mut agents = self.agents.lock().unwrap_or_else(|e| e.into_inner());
            agents.retain(|_, weak| weak.strong_count() > 0);
            agents
                .remove(id)
                .and_then(|weak| weak.upgrade())
                .ok_or_else(|| AgentError::UnknownId(id.to_string()))?
        };
        log::debug!("[embed][runtime] removing agent id={id} purge={purge}");
        inner.lifecycle.mark_removed();
        if !inner.lifecycle.wait_idle(REMOVE_IDLE_WAIT).await {
            log::warn!(
                "[embed][runtime] agent id={id} still had turns unwinding after {}s",
                REMOVE_IDLE_WAIT.as_secs()
            );
        }
        inner.teardown("agent_removed");
        if purge {
            let home = inner.layout.home.clone();
            if home.exists() {
                std::fs::remove_dir_all(&home).map_err(|source| AgentError::Workspace {
                    what: "delete the removed agent's home",
                    source,
                })?;
            }
        }
        log::debug!("[embed][runtime] agent removed id={id}");
        Ok(())
    }
}
