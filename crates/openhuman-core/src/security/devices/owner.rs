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

/// A scope's device lookup failed, and no other scope has the device, so its
/// owner is unknown. The frame is dropped rather than handled as `local`:
/// guessing could run a paired device's RPCs as the wrong agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OwnerLookupFailed {
    /// The scope whose lookup failed (`None` = `local`) and why.
    pub(super) agent: Option<String>,
    pub(super) error: String,
}

/// The agent `channel_id` belongs to, resolved as described above: `None`
/// for `local`, which is also where a channel no scope knows yet (a handshake
/// still in flight) is handled.
///
/// # Errors
///
/// [`OwnerLookupFailed`] when no scope has the device and at least one
/// scope's lookup failed.
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
    let lookups = crate::storage::agents::for_each_scope("device owner", || async {
        let config = crate::config::rpc::load_config_with_timeout()
            .await
            .map_err(|error| format!("load config: {error}"))?;
        super::store::get_device(&config, channel_id)
            .map(|device| device.is_some())
            .map_err(|error| error.to_string())
    })
    .await;
    let owner = decide(lookups)?;
    if let Some(owner) = &owner {
        log::debug!(
            "[devices/owner] channel_id={channel_id} belongs to agent={}",
            owner.as_deref().unwrap_or("local")
        );
        remember(channel_id, owner.clone());
    }
    Ok(owner.flatten())
}

/// The owner from each scope's lookup: the first scope that has the device
/// (`Some(Some(agent))` / `Some(None)` for `local`); `None` when none has it
/// and every lookup succeeded; an error when none has it and one failed.
fn decide(
    lookups: Vec<(Option<String>, Result<bool, String>)>,
) -> Result<Option<Option<String>>, OwnerLookupFailed> {
    let mut failure = None;
    for (agent, lookup) in lookups {
        match lookup {
            Ok(true) => return Ok(Some(agent)),
            Ok(false) => {}
            Err(error) => {
                failure.get_or_insert(OwnerLookupFailed { agent, error });
            }
        }
    }
    match failure {
        Some(failure) => Err(failure),
        None => Ok(None),
    }
}

#[cfg(test)]
#[path = "owner_tests.rs"]
mod tests;
