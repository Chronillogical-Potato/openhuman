//! Which agent a flow belongs to, for the subscribers that act on a flow
//! named only by id in an event (`FlowScheduleTick`, `FlowRunFinished`).
//!
//! A flow saved from inside an agent's context lives in that agent's storage
//! scope (`crate::storage`). The subscribers run outside any agent, so they
//! look the flow up here and handle the event as its owner
//! (`crate::storage::agents::within_agent`); without a storage backend every
//! flow is `local` and this is a no-op.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use crate::config::Config;

/// Flows whose owner this process has resolved (`None` = `local`).
static OWNERS: LazyLock<Mutex<HashMap<String, Option<String>>>> = LazyLock::new(Default::default);

/// The agent `flow_id` belongs to: `None` for `local`, or when no scope has
/// the flow (the handler then reports the flow as unknown, as before).
pub(super) async fn flow_owner(config: &Config, flow_id: &str) -> Option<String> {
    // No backend: every record is `local`.
    crate::storage::installed()?;
    resolve(config, flow_id).await
}

/// [`flow_owner`]'s cache and lookup, across whatever scopes exist.
async fn resolve(config: &Config, flow_id: &str) -> Option<String> {
    let cached = OWNERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(flow_id)
        .cloned();
    if let Some(owner) = cached {
        // Still there? A removed flow's id may be reused, or the flow moved:
        // re-check the cached scope and resolve again when it lost the flow.
        let still_there = crate::storage::agents::within_agent(owner.as_deref(), async {
            matches!(crate::flows::store::get_flow(config, flow_id), Ok(Some(_)))
        })
        .await;
        if still_there {
            return owner;
        }
        forget(flow_id);
    }
    let found = crate::storage::agents::find_owner("flow owner", || async {
        matches!(crate::flows::store::get_flow(config, flow_id), Ok(Some(_)))
    })
    .await;
    let owner = found?;
    tracing::debug!(
        target: "flows",
        %flow_id,
        agent = owner.as_deref().unwrap_or("local"),
        "[flows] resolved the flow's owner"
    );
    OWNERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(flow_id.to_string(), owner.clone());
    owner
}

/// Forgets a cached owner (a removed flow's id may be reused).
pub(super) fn forget(flow_id: &str) {
    OWNERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(flow_id);
}

#[cfg(test)]
#[path = "owner_tests.rs"]
mod tests;
