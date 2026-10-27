//! Event bus handlers for the devices domain.
//!
//! The host owns the socket transport. TinySecurity owns pairing identities,
//! handshake state, session ciphers and every per-frame crypto operation.

use crate::core::runtime::DispatchCtx;
use std::sync::{Arc, OnceLock};

use crate::core::bus::BUS;
use crate::core::events::DomainEvent;
use crate::security::devices::rpc::{
    ACTIVE_SESSIONS, PEER_STATUS, PENDING_RENEWALS, PENDING_SESSIONS, RECONNECTING,
};
use crate::security::devices::store;
use crate::security::devices::tunnel_client::emit_frame;
use async_trait::async_trait;
use serde_json::Value;
use tinybus::EventHandler;
use tinybus::SubscriptionHandle;

const HANDSHAKE_VERSION: u8 = 0x03;

static DEVICE_TUNNEL_HANDLE: OnceLock<SubscriptionHandle> = OnceLock::new();

/// Register the device tunnel subscriber on the global event bus.
/// Idempotent — subsequent calls are no-ops.
pub fn register_device_tunnel_subscriber() {
    if DEVICE_TUNNEL_HANDLE.get().is_some() {
        return;
    }
    match crate::core::bus::BUS.subscribe(Arc::new(DeviceTunnelSubscriber::new())) {
        Some(handle) => {
            let _ = DEVICE_TUNNEL_HANDLE.set(handle);
            log::info!("[devices/bus] DeviceTunnelSubscriber registered");
        }
        None => {
            log::warn!(
                "[devices/bus] failed to register DeviceTunnelSubscriber — bus not initialized"
            );
        }
    }
}

/// Subscribes to device tunnel events from the event bus.
pub struct DeviceTunnelSubscriber;

impl DeviceTunnelSubscriber {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DeviceTunnelSubscriber {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventHandler<DomainEvent> for DeviceTunnelSubscriber {
    fn name(&self) -> &str {
        "device::tunnel"
    }

    fn domains(&self) -> Option<&[&str]> {
        Some(&["device"])
    }

    async fn handle(&self, event: &DomainEvent) {
        match event {
            DomainEvent::DevicePeerOnline { channel_id } => {
                handle_peer_online(channel_id).await;
            }
            DomainEvent::DevicePeerOffline { channel_id } => {
                handle_peer_offline(channel_id).await;
            }
            DomainEvent::DeviceTunnelFrame {
                channel_id,
                payload_b64,
                connection_token,
                connection_nonce,
                expires_at_ms,
            } => {
                let pending = PENDING_SESSIONS
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get(channel_id.as_str())
                    .cloned();
                let host_ctx = DispatchCtx::host_default();
                match super::owner::owner_of(&host_ctx, channel_id, pending.as_ref()).await {
                    Ok(owner) => {
                        let handled = crate::storage::agents::within_agent(
                            &host_ctx,
                            owner.as_deref(),
                            async {
                                let owner_ctx = DispatchCtx::ambient().with_socket_event();
                                handle_tunnel_frame(
                                    &owner_ctx,
                                    channel_id,
                                    payload_b64,
                                    connection_token,
                                    connection_nonce,
                                    *expires_at_ms,
                                )
                                .await
                            },
                        )
                        .await;
                        if handled.is_none() {
                            log::warn!(
                                "[devices/bus] dropping tunnel frame channel_id={channel_id}: \n                                 its owner has no context to act under"
                            );
                        }
                    }
                    Err(failed) => log::warn!(
                        "[devices/bus] dropping tunnel frame channel_id={channel_id}: owner \n                         lookup failed in scope={} ({})",
                        failed.agent.as_deref().unwrap_or("local"),
                        failed.error
                    ),
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn handle_peer_online(channel_id: &str) {
    log::info!("[devices/bus] peer online channel_id={}", channel_id);
    super::lock_unpoisoned(&PEER_STATUS).insert(channel_id.to_string(), true);
    // No re-publish: the event was already published by socket::event_handlers.
}

async fn handle_peer_offline(channel_id: &str) {
    log::info!("[devices/bus] peer offline channel_id={}", channel_id);
    super::lock_unpoisoned(&PEER_STATUS).insert(channel_id.to_string(), false);
    // This backend event carries only a channel id, not the closed socket's
    // connection nonce. Keep the module session until expiry rather than
    // attesting a close against whichever connection is current now.
}

/// Route a socket frame through TinySecurity and preserve the existing wire envelope.
async fn handle_tunnel_frame(
    ctx: &DispatchCtx,
    channel_id: &str,
    payload_b64: &str,
    connection_token: &str,
    connection_nonce: &str,
    expires_at_ms: u64,
) {
    let Some(frame_bytes) = decode_frame(payload_b64) else {
        log::warn!("[devices/bus] invalid tunnel frame encoding channel_id={channel_id}");
        return;
    };
    let Ok(config) = ctx.load_config().await else {
        return;
    };
    if frame_bytes.first() == Some(&HANDSHAKE_VERSION) {
        handle_handshake(
            channel_id,
            &config,
            &frame_bytes,
            connection_token,
            connection_nonce,
            expires_at_ms,
        )
        .await;
    } else if frame_bytes.first() == Some(&0x02) {
        handle_encrypted_rpc_frame(
            ctx,
            channel_id,
            &config,
            &frame_bytes,
            connection_nonce,
            expires_at_ms,
        )
        .await;
    } else {
        log::warn!("[devices/bus] unsupported frame version channel_id={channel_id}");
    }
}

async fn handle_handshake(
    channel_id: &str,
    config: &crate::config::Config,
    frame: &[u8],
    connection_token: &str,
    connection_nonce: &str,
    expires_at_ms: u64,
) {
    let pending = PENDING_SESSIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(channel_id)
        .cloned();
    let session = match pending {
        Some(session) => session,
        None => {
            let Some(session) = begin_device_renewal(
                channel_id,
                config,
                connection_token,
                connection_nonce,
                expires_at_ms,
            )
            .await
            else {
                log::warn!("[devices/bus] no pending pairing or renew authority for channel_id={channel_id}");
                return;
            };
            session
        }
    };
    if session.channel_id != channel_id || !pairing_session_is_live(&session) {
        return;
    }
    let Ok((event_id, _scope_id)) = crate::modules::security::device_pairing_socket_event(
        config,
        channel_id,
        &session.pairing_token,
        connection_nonce,
        expires_at_ms,
    )
    .await
    else {
        return;
    };
    let Ok(client) = crate::modules::security::client(config).await else {
        return;
    };
    let result = client
        .device_pairing2(tinysecurity_bus::DevicePairingRequest2 {
            call_id: uuid::Uuid::new_v4().simple().to_string(),
            operation: tinysecurity_bus::DevicePairingOperation2::Handshake {
                receipt: session.pairing_receipt.clone(),
                host_event_id: event_id,
                frame: frame.to_vec(),
            },
        })
        .await;
    let Ok(result) = result else {
        log::warn!("[devices/bus] TinySecurity rejected handshake channel_id={channel_id}");
        return;
    };
    let Some(ack) = valid_handshake_reply(result) else {
        log::warn!("[devices/bus] malformed TinySecurity handshake result channel_id={channel_id}");
        return;
    };
    if let Err(error) = emit_frame(channel_id, &encode_frame(&ack)).await {
        log::warn!("[devices/bus] handshake ACK emit failed channel_id={channel_id}: {error}");
        return;
    }
    PENDING_HANDSHAKES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(channel_id.to_owned());
}

async fn begin_device_renewal(
    channel_id: &str,
    config: &crate::config::Config,
    connection_token: &str,
    connection_nonce: &str,
    expires_at_ms: u64,
) -> Option<super::types::PairingSession> {
    if !PEER_STATUS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(channel_id)
        .copied()
        .unwrap_or(false)
    {
        return None;
    }
    if !RECONNECTING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(channel_id.to_owned())
    {
        return None;
    }

    let renewed = begin_device_renewal_once(
        channel_id,
        config,
        connection_token,
        connection_nonce,
        expires_at_ms,
    )
    .await;
    RECONNECTING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(channel_id);
    renewed
}

async fn begin_device_renewal_once(
    channel_id: &str,
    config: &crate::config::Config,
    connection_token: &str,
    connection_nonce: &str,
    expires_at_ms: u64,
) -> Option<super::types::PairingSession> {
    let device = match store::get_device(config, channel_id) {
        Ok(Some(device)) if !device.revoked => device,
        _ => return None,
    };
    let is_pending = PENDING_SESSIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains_key(channel_id);
    let active_is_current = ACTIVE_SESSIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(channel_id)
        .is_some_and(|active| active_session_matches_connection(active, connection_nonce));
    let peer_online = PEER_STATUS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(channel_id)
        .copied()
        .unwrap_or(false);
    let scope_unlocked = crate::modules::security::device_pairing_scope_is_unlocked(config).await;
    let pairing_id = renewal_pairing_id(
        peer_online,
        scope_unlocked,
        is_pending,
        active_is_current,
        &device,
    )?;
    let event_id = crate::modules::security::device_pairing_reconnect_event(
        config,
        channel_id,
        connection_token,
        connection_nonce,
        expires_at_ms,
    )
    .await
    .ok()?;
    let client = crate::modules::security::client(config).await.ok()?;
    let response = client
        .device_pairing2(tinysecurity_bus::DevicePairingRequest2 {
            call_id: uuid::Uuid::new_v4().simple().to_string(),
            operation: tinysecurity_bus::DevicePairingOperation2::Renew {
                pairing_id: pairing_id.to_owned(),
                host_event_id: event_id,
            },
        })
        .await
        .ok()?;
    let session = renewal_pairing_session(
        channel_id,
        connection_token,
        &device.label,
        response,
        expires_at_ms,
    )?;
    PENDING_RENEWALS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(channel_id.to_owned(), pairing_id.to_owned());
    PENDING_SESSIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(channel_id.to_owned(), session.clone());
    Some(session)
}

fn renewal_pairing_id<'a>(
    peer_online: bool,
    scope_unlocked: bool,
    has_pending_session: bool,
    has_current_session: bool,
    device: &'a super::types::PairedDevice,
) -> Option<&'a str> {
    if !peer_online
        || !scope_unlocked
        || has_pending_session
        || has_current_session
        || device.revoked
    {
        return None;
    }
    device.pairing_id.as_deref().filter(|value| !value.is_empty())
}

fn renewal_pairing_session(
    channel_id: &str,
    connection_token: &str,
    label: &str,
    response: tinysecurity_bus::DevicePairingResponse2,
    attestation_expiry_ms: u64,
) -> Option<super::types::PairingSession> {
    let receipt = response.receipt.filter(|value| !value.is_empty())?;
    let core_pubkey = response.core_public_key.filter(|value| !value.is_empty())?;
    if response.pairing_id.is_some()
        || response.session_id.is_some()
        || response.device_public_key.is_some()
        || !response.frame.is_empty()
        || response.expires_at_ms <= unix_time_ms()
        || response.expires_at_ms > attestation_expiry_ms
    {
        return None;
    }
    let expires_at = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(
        i64::try_from(response.expires_at_ms).ok()?,
    )?
    .to_rfc3339();
    Some(super::types::PairingSession {
        channel_id: channel_id.to_owned(),
        pairing_token: connection_token.to_owned(),
        core_pubkey,
        pairing_receipt: receipt,
        label: label.to_owned(),
        rpc_url: None,
        expires_at,
        agent: None,
    })
}

fn renewal_device_matches(
    existing: &super::types::PairedDevice,
    expected_pairing_id: &str,
    returned_pairing_id: &str,
    returned_device_key: &str,
) -> bool {
    !existing.revoked
        && existing.pairing_id.as_deref() == Some(expected_pairing_id)
        && returned_pairing_id == expected_pairing_id
        && existing.device_pubkey == returned_device_key
}

fn active_session_matches_connection(
    active: &super::rpc::ActiveDeviceSession,
    connection_nonce: &str,
) -> bool {
    active.connection_nonce == connection_nonce
}

fn valid_handshake_reply(
    response: tinysecurity_bus::DevicePairingResponse2,
) -> Option<Vec<u8>> {
    if response.frame.is_empty()
        || response.receipt.is_some()
        || response.pairing_id.is_some()
        || response.session_id.is_some()
        || response.core_public_key.is_some()
        || response.device_public_key.is_some()
    {
        None
    } else {
        Some(response.frame)
    }
}

fn unix_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}

static PENDING_HANDSHAKES: once_cell::sync::Lazy<
    std::sync::Mutex<std::collections::HashSet<String>>,
> = once_cell::sync::Lazy::new(Default::default);

fn pairing_session_is_live(session: &super::types::PairingSession) -> bool {
    chrono::DateTime::parse_from_rfc3339(&session.expires_at)
        .ok()
        .is_some_and(|expiry| expiry.timestamp_millis() > chrono::Utc::now().timestamp_millis())
}

async fn handle_encrypted_rpc_frame(
    ctx: &DispatchCtx,
    channel_id: &str,
    config: &crate::config::Config,
    frame: &[u8],
    connection_nonce: &str,
    expires_at_ms: u64,
) {
    let pending = PENDING_SESSIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(channel_id)
        .cloned();
    if PENDING_HANDSHAKES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains(channel_id)
    {
        let Some(session) = pending else { return };
        confirm_pairing(
            channel_id,
            ctx.tenant().ok().and_then(|tenant| tenant.agent),
            config,
            &session,
            frame,
            connection_nonce,
            expires_at_ms,
        )
        .await;
        return;
    }
    let active = ACTIVE_SESSIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(channel_id)
        .cloned();
    let Some(active) = active else {
        log::warn!("[devices/bus] encrypted frame without active session channel_id={channel_id}");
        return;
    };
    if !active_session_matches_connection(&active, connection_nonce) {
        ACTIVE_SESSIONS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(channel_id);
        log::warn!("[devices/bus] active session belongs to an older socket channel_id={channel_id}");
        return;
    }
    let Ok(client) = crate::modules::security::client(config).await else {
        return;
    };
    let Ok((event_id, _scope_id)) = crate::modules::security::device_pairing_socket_event(
        config,
        channel_id,
        &active.pairing_token,
        connection_nonce,
        expires_at_ms,
    )
    .await
    else {
        return;
    };
    let refreshed = client
        .device_pairing2(tinysecurity_bus::DevicePairingRequest2 {
            call_id: uuid::Uuid::new_v4().simple().to_string(),
            operation: tinysecurity_bus::DevicePairingOperation2::RefreshSession {
                session_id: active.session_id.clone(),
                host_event_id: event_id,
            },
        })
        .await;
    if !refreshed.is_ok_and(|response| response.session_id.as_deref() == Some(&active.session_id)) {
        log::warn!("[devices/bus] TinySecurity could not refresh session channel_id={channel_id}");
        ACTIVE_SESSIONS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(channel_id);
        return;
    }
    let opened = client
        .device_frame2(tinysecurity_bus::DeviceFrameRequest2 {
            scope_id: active.scope_id.clone(),
            handle: active.session_id.clone(),
            call_id: uuid::Uuid::new_v4().simple().to_string(),
            operation: tinysecurity_bus::DeviceFrameOperation2::Open {
                frame: frame.to_vec(),
            },
        })
        .await;
    let Ok(opened) = opened else {
        log::warn!("[devices/bus] TinySecurity rejected frame channel_id={channel_id}");
        return;
    };
    let envelope = match serde_json::from_slice::<super::tunnel_rpc::TunnelEnvelope>(&opened.bytes)
    {
        Ok(envelope) => envelope,
        Err(error) => {
            log::warn!(
                "[devices/bus] tunnel envelope parse failed channel_id={channel_id}: {error}"
            );
            return;
        }
    };
    if envelope.kind != "request" {
        return;
    }
    let request = match super::tunnel_rpc::parse_tunnel_rpc_payload(envelope.payload) {
        Ok(request) => request,
        Err(error) => {
            send_tunnel_response(
                channel_id,
                config,
                &active,
                connection_nonce,
                expires_at_ms,
                &envelope.request_id,
                "error",
                Value::String(format!("invalid tunnel RPC payload: {error}")),
            )
            .await;
            return;
        }
    };
    let result = super::tunnel_rpc::dispatch_tunnel_rpc(ctx, request).await;
    match result {
        Ok(value) => {
            send_tunnel_response(
                channel_id,
                config,
                &active,
                connection_nonce,
                expires_at_ms,
                &envelope.request_id,
                "response",
                value,
            )
            .await
        }
        Err(message) => {
            send_tunnel_response(
                channel_id,
                config,
                &active,
                connection_nonce,
                expires_at_ms,
                &envelope.request_id,
                "error",
                Value::String(message),
            )
            .await
        }
    }
}

async fn confirm_pairing(
    channel_id: &str,
    agent: Option<String>,
    config: &crate::config::Config,
    session: &super::types::PairingSession,
    frame: &[u8],
    connection_nonce: &str,
    expires_at_ms: u64,
) {
    let Ok((event_id, scope_id)) = crate::modules::security::device_pairing_socket_event(
        config,
        channel_id,
        &session.pairing_token,
        connection_nonce,
        expires_at_ms,
    )
    .await
    else {
        return;
    };
    let Ok(client) = crate::modules::security::client(config).await else {
        return;
    };
    let result = client
        .device_pairing2(tinysecurity_bus::DevicePairingRequest2 {
            call_id: uuid::Uuid::new_v4().simple().to_string(),
            operation: tinysecurity_bus::DevicePairingOperation2::Confirm {
                receipt: session.pairing_receipt.clone(),
                host_event_id: event_id,
                frame: frame.to_vec(),
            },
        })
        .await;
    let Ok(result) = result else {
        log::warn!(
            "[devices/bus] TinySecurity rejected pairing confirmation channel_id={channel_id}"
        );
        return;
    };
    let (Some(pairing_id), Some(session_id), Some(device_public_key)) = (
        result.pairing_id,
        result.session_id,
        result.device_public_key,
    ) else {
        log::warn!("[devices/bus] malformed TinySecurity confirmation channel_id={channel_id}");
        return;
    };
    if !result.frame.is_empty() {
        return;
    }
    let token_hash = hash_session_token(&session.pairing_token);
    let renewal_pairing_id = PENDING_RENEWALS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(channel_id)
        .cloned();
    let persisted = if let Some(expected_pairing_id) = renewal_pairing_id.as_deref() {
        match store::get_device(config, channel_id) {
            Ok(Some(existing))
                if renewal_device_matches(
                    &existing,
                    expected_pairing_id,
                    &pairing_id,
                    &device_public_key,
                ) =>
            {
                store::touch_device(config, channel_id).and_then(|()| {
                    store::get_device(config, channel_id)
                        .map(|device| device.ok_or_else(|| anyhow::anyhow!("renewed device disappeared")))
                        .and_then(|device| device)
                })
            }
            _ => Err(anyhow::anyhow!("renewed device metadata mismatch")),
        }
    } else {
        store::insert_device_with_pairing_id(
            config,
            channel_id,
            &session.label,
            &device_public_key,
            &token_hash,
            &pairing_id,
        )
    };
    match persisted {
        Ok(device) => {
            super::owner::remember(channel_id, agent);
            BUS.publish(DomainEvent::DevicePaired {
                channel_id: channel_id.to_owned(),
                device_pubkey: device_public_key,
                label: Some(device.label),
            });
            ACTIVE_SESSIONS
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(
                    channel_id.to_owned(),
                    super::rpc::ActiveDeviceSession {
                        scope_id,
                        session_id,
                        pairing_token: session.pairing_token.clone(),
                        connection_nonce: connection_nonce.to_owned(),
                    },
                );
            PENDING_SESSIONS
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(channel_id);
            PENDING_RENEWALS
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(channel_id);
            PENDING_HANDSHAKES
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(channel_id);
            let _ = device;
            log::info!(
                "[devices/bus] pairing confirmed channel_id={channel_id} pairing_id={pairing_id}"
            );
        }
        Err(error) => {
            log::error!("[devices/bus] could not persist paired device {channel_id}: {error}")
        }
    }
}

async fn send_tunnel_response(
    channel_id: &str,
    config: &crate::config::Config,
    session: &super::rpc::ActiveDeviceSession,
    _connection_nonce: &str,
    _expires_at_ms: u64,
    request_id: &str,
    kind: &str,
    payload: Value,
) {
    let response = super::tunnel_rpc::response_envelope(request_id, kind, payload);
    let Ok(plaintext) = serde_json::to_vec(&response) else {
        return;
    };
    let Ok(client) = crate::modules::security::client(config).await else {
        return;
    };
    let sealed = client
        .device_frame2(tinysecurity_bus::DeviceFrameRequest2 {
            scope_id: session.scope_id.clone(),
            handle: session.session_id.clone(),
            call_id: uuid::Uuid::new_v4().simple().to_string(),
            operation: tinysecurity_bus::DeviceFrameOperation2::Seal { plaintext },
        })
        .await;
    let Ok(sealed) = sealed else {
        log::error!("[devices/bus] TinySecurity failed to seal response channel_id={channel_id}");
        return;
    };
    if let Err(error) = emit_frame(channel_id, &encode_frame(&sealed.bytes)).await {
        log::error!("[devices/bus] tunnel response emit failed channel_id={channel_id}: {error}");
    }
}

fn decode_frame(value: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(value)
        .ok()
}

fn encode_frame(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub(super) fn abandon_unpersisted_pairing(channel_id: &str) {
    PENDING_SESSIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(channel_id);
    PENDING_RENEWALS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(channel_id);
    PENDING_HANDSHAKES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(channel_id);
    ACTIVE_SESSIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(channel_id);
    PENDING_RENEWALS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(channel_id);
    RECONNECTING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(channel_id);
}

fn hash_session_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
#[path = "bus_tests.rs"]
mod tests;
