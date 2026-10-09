//! Which agent a paired device belongs to.
//!
//! A device paired from inside an agent's context is stored in that agent's
//! storage scope (`crate::storage`), and the RPCs it sends through the tunnel
//! must run as that agent — not as the process default, which would read and
//! write somebody else's records. The tunnel subscriber runs outside any
//! agent, so it asks here, per frame:
//!
//! 1. the pending pairing session, which recorded the agent that started it;
//! 2. the owners this process has already resolved;
//! 3. otherwise each storage scope, for a device paired by an earlier process
//!    (`crate::storage::agents::for_each_scope`).
//!
//! `None` means the `local` scope (a single-user host, or no backend).

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use super::types::PairingSession;

/// Channels whose owner this process has resolved: `Some(agent)`, or `None`
/// for `local`.
static OWNERS: LazyLock<Mutex<HashMap<String, Option<String>>>> =
    LazyLock::new(Default::default);

/// Records that `channel_id` belongs to `agent` (`None` = `local`).
pub(super) fn remember(channel_id: &str, agent: Option<String>) {
    OWNERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(channel_id.to_string(), agent);
}

fn cached(channel_id: &str) -> Option<Option<String>> {
    OWNERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(channel_id)
        .cloned()
}

/// The agent `channel_id` belongs to, resolved as described above. A channel
/// no scope knows yet (a handshake still in flight) is `local`.
pub(super) async fn owner_of(
    channel_id: &str,
    pending: Option<&PairingSession>,
) -> Option<String> {
    if let Some(session) = pending {
        return session.agent.clone();
    }
    if let Some(owner) = cached(channel_id) {
        return owner;
    }
    let Ok(config) = crate::config::rpc::load_config_with_timeout().await else {
        return None;
    };
    let found = crate::storage::agents::for_each_scope("device owner", || async {
        super::store::get_device(&config, channel_id)
            .ok()
            .flatten()
            .is_some()
    })
    .await
    .into_iter()
    .find_map(|(agent, has_device)| has_device.then_some(agent));
    match found {
        Some(owner) => {
            log::debug!(
                "[devices/owner] channel_id={channel_id} belongs to agent={}",
                owner.as_deref().unwrap_or("local")
            );
            remember(channel_id, owner.clone());
            owner
        }
        None => None,
    }
}

#[cfg(test)]
#[path = "owner_tests.rs"]
mod tests;
