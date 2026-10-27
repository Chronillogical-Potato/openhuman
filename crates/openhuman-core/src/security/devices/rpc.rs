//! RPC handler implementations for the devices domain.
//!
//! Three methods:
//!  - `devices_create_pairing` — registers a pairing channel and returns QR fields.
//!  - `devices_list`           — lists non-revoked paired devices.
//!  - `devices_revoke`         — marks a device revoked and closes its tunnel channel.
//!
//! Keypair persistence: private key bytes are encrypted with the workspace
//! `SecretStore` and stored in the paired-device document. Restarted hosts
//! reconstruct the stable identity while deriving a fresh in-memory tunnel.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Mutex;

use crate::config::Config;
use crate::core::Outcome;
use crate::security::devices::store;
use crate::security::devices::tunnel_client;
use crate::security::devices::types::{
    CreatePairingResponse, ImportLegacyResponse, ListDevicesResponse, PairingSession,
    RevokeDeviceResponse,
};

fn require_user_controller(ctx: &crate::core::runtime::DispatchCtx) -> Result<(), String> {
    if ctx.invocation_source()
        == crate::core::runtime::InvocationSource::AuthenticatedUserController
    {
        Ok(())
    } else {
        Err("device pairing requires a direct authenticated user controller".into())
    }
}

// ---------------------------------------------------------------------------
// In-memory state (module-level singletons)
// ---------------------------------------------------------------------------

/// Pairing sessions pending device connection (keyed by channel_id).
pub(crate) static PENDING_SESSIONS: once_cell::sync::Lazy<Mutex<HashMap<String, PairingSession>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

/// Live peer-online status (keyed by channel_id). Updated by bus.rs on `tunnel:peer-status`.
pub(crate) static PEER_STATUS: once_cell::sync::Lazy<Mutex<HashMap<String, bool>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

/// Active module-owned tunnel sessions (keyed by channel_id).
pub(crate) static ACTIVE_SESSIONS: once_cell::sync::Lazy<
    Mutex<HashMap<String, ActiveDeviceSession>>,
> = once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ActiveDeviceSession {
    pub scope_id: String,
    pub session_id: String,
    pub pairing_token: String,
}

// ---------------------------------------------------------------------------
// create_pairing
// ---------------------------------------------------------------------------

/// `openhuman.devices_create_pairing`
///
/// 1. Calls `tunnel:register` on the shared socket — backend returns
///    `{channelId, pairingToken, pairingExpiresAt}` via Socket.IO ACK.
/// 2. Asks TinySecurity to mint the core identity and a one-use owner receipt.
/// 3. Emits `tunnel:connect` with `role:"core"` so the core starts listening.
/// 4. Detects the local LAN IP for the optional direct fast-path `rpc_url`.
/// 5. Returns QR-bound fields to the caller.
pub async fn devices_create_pairing(
    ctx: &crate::core::runtime::DispatchCtx,
    _config: &Config,
    label: Option<String>,
) -> Result<Outcome<CreatePairingResponse>, String> {
    require_user_controller(ctx)?;
    log::info!(
        "[devices/rpc] devices_create_pairing entry label={:?}",
        label
    );

    // Key generation and private identity storage stay inside TinySecurity.
    let (client, scope, master_handle, generation) =
        crate::modules::security::device_pairing_master(_config).await?;
    // The module binds Begin to the backend-issued channel and pairing token,
    // so the authenticated user action is minted after tunnel registration.
    let reg = tunnel_client::emit_register().await.map_err(|e| {
        log::error!("[devices/rpc] tunnel:register failed: {e}");
        e
    })?;

    log::info!(
        "[devices/rpc] tunnel:register ok channel_id={} token_len={}",
        reg.channel_id,
        reg.pairing_token.len()
    );

    let host_event_id = crate::modules::security::device_pairing_controller_event(
        &scope,
        master_handle,
        generation,
        reg.channel_id.clone(),
        reg.pairing_token.clone(),
    )?;
    let begin = client
        .device_pairing2(pairing_begin_request(host_event_id))
        .await
        .map_err(|error| error.to_string())?;
    let (core_pubkey, pairing_receipt) = pairing_begin_result(begin)?;

    // Best-effort LAN URL detection (non-fatal if it fails).
    let rpc_url = detect_lan_rpc_url();
    if let Some(ref url) = rpc_url {
        log::debug!("[devices/rpc] LAN rpc_url detected: {}", url);
    }

    let expires_at = reg.pairing_expires_at.clone();

    // Insert the pending session BEFORE opening the tunnel: `emit_connect`
    // starts `tunnel:frame` handling, and `bus.rs` now derives the persisted
    // pairing credential from this map, so a fast inbound frame must never race
    // ahead of the entry (CodeRabbit #4355).
    super::lock_unpoisoned(&PENDING_SESSIONS).insert(
        reg.channel_id.clone(),
        PairingSession {
            channel_id: reg.channel_id.clone(),
            pairing_token: reg.pairing_token.clone(),
            core_pubkey: core_pubkey.clone(),
            pairing_receipt,
            label: label
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| reg.channel_id.clone()),
            rpc_url: rpc_url.clone(),
            expires_at: expires_at.clone(),
            agent: ctx.tenant().ok().and_then(|tenant| tenant.agent),
        },
    );

    // Connect as "core" role to start listening on this channel.
    connect_after_reserving_pairing(
        &reg.channel_id,
        tunnel_client::emit_connect(&reg.channel_id),
    )
    .await
    .map_err(|e| {
        log::error!("[devices/rpc] tunnel:connect failed: {e}");
        e
    })?;

    log::debug!(
        "[devices/rpc] tunnel:connect emitted channel_id={}",
        reg.channel_id
    );

    log::info!(
        "[devices/rpc] devices_create_pairing done channel_id={}",
        reg.channel_id
    );

    Ok(Outcome::single_log(
        CreatePairingResponse {
            channel_id: reg.channel_id,
            pairing_token: reg.pairing_token,
            core_pubkey,
            rpc_url,
            expires_at,
        },
        "pairing channel created",
    ))
}

// ---------------------------------------------------------------------------
// legacy import
// ---------------------------------------------------------------------------

/// Import available pre-TinySecurity identities after a direct user approval.
pub async fn devices_import_legacy(
    ctx: &crate::core::runtime::DispatchCtx,
    config: &Config,
) -> Result<Outcome<ImportLegacyResponse>, String> {
    require_user_controller(ctx)?;
    let host_event_id = crate::modules::security::device_pairing_legacy_import_event(config).await?;
    let client = crate::modules::security::client(config)
        .await
        .map_err(|error| error.to_string())?;
    let imported = client
        .device_pairing2(tinysecurity_bus::DevicePairingRequest2 {
            call_id: uuid::Uuid::new_v4().simple().to_string(),
            operation: tinysecurity_bus::DevicePairingOperation2::ImportLegacy { host_event_id },
        })
        .await
        .map_err(|error| error.to_string())?;
    if !imported.frame.is_empty()
        || imported.pairing_id.is_some()
        || imported.session_id.is_some()
        || imported.core_public_key.is_some()
        || imported.device_public_key.is_some()
    {
        return Err("TinySecurity returned an unexpected legacy import result".to_owned());
    }

    let rows = store::list_devices_for_legacy_import(config)
        .map_err(|error| format!("[devices/rpc] legacy device list failed: {error}"))?;
    let mut reconciled = 0;
    for row in rows.into_iter().filter(|row| !row.revoked) {
        if row.pairing_id.is_some() || row.core_private_key_encrypted.is_none() {
            continue;
        }
        let scope = crate::modules::security::host_callbacks::secret_scope()
            .await
            .ok_or_else(|| "security secret scope unavailable".to_owned())?;
        let generation = client
            .policy_info()
            .await
            .map_err(|error| error.to_string())?
            .generation;
        let event_id = crate::modules::security::device_pairing_user_event(
            &scope,
            String::new(),
            generation,
        )?;
        let recovered = client
            .device_pairing2(tinysecurity_bus::DevicePairingRequest2 {
                call_id: uuid::Uuid::new_v4().simple().to_string(),
                operation: tinysecurity_bus::DevicePairingOperation2::Recover {
                    channel_id: row.channel_id.clone(),
                    host_event_id: event_id,
                },
            })
            .await
            .map_err(|error| error.to_string())?;
        let Some(pairing_id) = recovered.pairing_id.filter(|id| !id.is_empty()) else {
            return Err("TinySecurity did not recover the imported device id".to_owned());
        };
        if recovered.device_public_key.as_deref() != Some(row.device_pubkey.as_str())
            || recovered.frame.len() > 0
            || recovered.session_id.is_some()
        {
            return Err("TinySecurity returned mismatched imported device metadata".to_owned());
        }
        if !store::set_pairing_id(config, &row.channel_id, &pairing_id)
            .map_err(|error| format!("[devices/rpc] pairing id persistence failed: {error}"))?
        {
            return Err("legacy device disappeared during pairing id persistence".to_owned());
        }
        reconciled += 1;
    }
    Ok(Outcome::single_log(
        ImportLegacyResponse {
            imported: reconciled,
        },
        "legacy device identities imported",
    ))
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

/// `openhuman.devices_list`
pub async fn devices_list(config: &Config) -> Result<Outcome<ListDevicesResponse>, String> {
    log::debug!("[devices/rpc] devices_list entry");
    let mut devices = store::list_devices(config)
        .map_err(|e| format!("[devices/rpc] list_devices failed: {e}"))?;

    // Overlay live peer-online status from in-memory map.
    {
        let peer_map = super::lock_unpoisoned(&PEER_STATUS);
        for dev in &mut devices {
            let online = peer_map.get(&dev.channel_id).copied().unwrap_or(false);
            dev.peer_online = Some(online);
        }
    }

    log::debug!(
        "[devices/rpc] devices_list returning {} device(s)",
        devices.len()
    );
    Ok(Outcome::new(ListDevicesResponse { devices }, vec![]))
}

// ---------------------------------------------------------------------------
// revoke
// ---------------------------------------------------------------------------

/// `openhuman.devices_revoke`
pub async fn devices_revoke(
    ctx: &crate::core::runtime::DispatchCtx,
    config: &Config,
    channel_id: String,
) -> Result<Outcome<RevokeDeviceResponse>, String> {
    require_user_controller(ctx)?;
    log::info!("[devices/rpc] devices_revoke channel_id={}", channel_id);

    let revoked = store::revoke_device(config, &channel_id)
        .map_err(|e| format!("[devices/rpc] revoke_device failed: {e}"))?;

    // Clear in-memory state for this channel.
    super::lock_unpoisoned(&PENDING_SESSIONS).remove(&channel_id);
    super::lock_unpoisoned(&PEER_STATUS).remove(&channel_id);
    super::lock_unpoisoned(&ACTIVE_SESSIONS).remove(&channel_id);

    // Publish DeviceRevoked so UI and other subscribers are notified.
    crate::core::bus::BUS.publish(crate::core::events::DomainEvent::DeviceRevoked {
        channel_id: channel_id.clone(),
    });

    // TODO: backend revoke endpoint pending (PR #709 follow-up).
    // For now, closing the local tunnel side + letting the backend TTL the channel is sufficient.
    log::info!(
        "[devices/rpc] devices_revoke done channel_id={} revoked={}",
        channel_id,
        revoked
    );

    Ok(Outcome::single_log(
        RevokeDeviceResponse { success: revoked },
        format!("device {channel_id} revoked"),
    ))
}

async fn connect_after_reserving_pairing<F>(channel_id: &str, connect: F) -> Result<(), String>
where
    F: Future<Output = Result<(), String>>,
{
    match connect.await {
        Ok(()) => Ok(()),
        Err(error) => {
            super::lock_unpoisoned(&PENDING_SESSIONS).remove(channel_id);
            Err(error)
        }
    }
}

// ---------------------------------------------------------------------------
// LAN URL detection
// ---------------------------------------------------------------------------

fn detect_lan_rpc_url() -> Option<String> {
    let ip = find_local_ipv4()?;
    // Use the configured RPC port if available via env, else fall back to 7788.
    let port = std::env::var("OPENHUMAN_CORE_RPC_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(7788);
    Some(format!("http://{}:{}/rpc", ip, port))
}

fn find_local_ipv4() -> Option<String> {
    use std::net::{IpAddr, UdpSocket};
    // UDP trick: connect to a public address (no packet sent) and read local addr.
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(addr) if !addr.is_loopback() => Some(addr.to_string()),
        _ => None,
    }
}

fn pairing_begin_result(
    response: tinysecurity_bus::DevicePairingResponse2,
) -> Result<(String, String), String> {
    let public_key = response
        .core_public_key
        .filter(|key| !key.is_empty())
        .ok_or_else(|| "TinySecurity did not return a core public key".to_owned())?;
    let receipt = response
        .receipt
        .filter(|receipt| !receipt.is_empty())
        .ok_or_else(|| "TinySecurity did not return a pairing receipt".to_owned())?;
    if !response.frame.is_empty() || response.pairing_id.is_some() || response.session_id.is_some()
    {
        return Err("TinySecurity returned an unexpected result for pairing begin".to_owned());
    }
    Ok((public_key, receipt))
}

fn pairing_begin_request(
    host_event_id: String,
) -> tinysecurity_bus::DevicePairingRequest2 {
    tinysecurity_bus::DevicePairingRequest2 {
        call_id: uuid::Uuid::new_v4().simple().to_string(),
        operation: tinysecurity_bus::DevicePairingOperation2::Begin { host_event_id },
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "rpc_tests.rs"]
mod tests;
