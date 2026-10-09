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
static OWNERS: LazyLock<Mutex<HashMap<String, Option<String>>>> = LazyLock::new(Default::default);

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
///
/// # Errors
///
/// [`OwnerLookupFailed`] when no scope claimed the channel and at least one
/// could not be searched (its configuration would not load): the device may
/// well belong to that scope, so the caller must refuse the frame instead of
/// running it as `local`.
pub(super) async fn owner_of(
    channel_id: &str,
    pending: Option<&PairingSession>,
) -> Result<Option<String>, OwnerLookupFailed> {
    if let Some(session) = pending {
        return Ok(session.agent.clone());
    }
    if let Some(owner) = cached(channel_id) {
        return Ok(owner);
    }
    // The configuration is loaded inside each scope: in SaaS mode loading it
    // needs an acting agent, which this tunnel task does not have.
    let found = crate::storage::agents::for_each_scope("device owner", || async {
        let Ok(config) = crate::config::rpc::load_config_with_timeout().await else {
            return None;
        };
        // A read error is not "not here": report the scope as unsearched.
        super::store::get_device(&config, channel_id)
            .ok()
            .map(|device| device.is_some())
    })
    .await;
    if let Some(owner) = found
        .iter()
        .find_map(|(agent, has_device)| (*has_device == Some(true)).then(|| agent.clone()))
    {
        log::debug!(
            "[devices/owner] channel_id={channel_id} belongs to agent={}",
            owner.as_deref().unwrap_or("local")
        );
        remember(channel_id, owner.clone());
        return Ok(owner);
    }
    if found.iter().any(|(_, has_device)| has_device.is_none()) {
        log::warn!("[devices/owner] channel_id={channel_id} could not be searched in every scope");
        return Err(OwnerLookupFailed);
    }
    Ok(None)
}

/// A scope's configuration would not load, so the owner of a channel could not
/// be ruled out there.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct OwnerLookupFailed;

#[cfg(test)]
#[path = "owner_tests.rs"]
mod tests;
