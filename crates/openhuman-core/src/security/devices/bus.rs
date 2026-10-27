//! Event bus handlers for the devices domain.
//!
//! The host owns the socket transport. TinySecurity owns pairing identities,
//! handshake state, session ciphers and every per-frame crypto operation.

use crate::core::runtime::DispatchCtx;
use std::sync::{Arc, OnceLock};

use crate::core::bus::BUS;
use crate::core::events::DomainEvent;
use crate::security::devices::rpc::{ACTIVE_SESSIONS, PEER_STATUS, PENDING_SESSIONS};
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
                handle_peer_offline(channel_id, &DispatchCtx::host_default()).await;
            }
            DomainEvent::DeviceTunnelFrame {
                channel_id,
                payload_b64,
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

async fn handle_peer_offline(channel_id: &str, ctx: &DispatchCtx) {
    log::info!("[devices/bus] peer offline channel_id={}", channel_id);
    super::lock_unpoisoned(&PEER_STATUS).insert(channel_id.to_string(), false);
    let attestation = super::tunnel_client::take_socket_attestation(channel_id);
    let active = ACTIVE_SESSIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(channel_id);
    if let (Some((nonce, expires_at_ms)), Some(active)) = (attestation, active) {
        if let Ok(config) = ctx.load_config().await {
            if let Ok((event_id, _)) =
                crate::modules::security::device_pairing_socket_closed_event(
                    &config,
                    channel_id,
                    &nonce,
                    expires_at_ms,
                )
                .await
            {
                if let Ok(client) = crate::modules::security::client(&config).await {
                    let _ = client
                        .device_pairing2(tinysecurity_bus::DevicePairingRequest2 {
                            call_id: uuid::Uuid::new_v4().simple().to_string(),
                            operation: tinysecurity_bus::DevicePairingOperation2::EndSession {
                                session_id: active.session_id,
                                host_event_id: event_id,
                            },
                        })
                        .await;
                }
            }
        }
    }
}

/// Route a socket frame through TinySecurity and preserve the existing wire envelope.
async fn handle_tunnel_frame(
    ctx: &DispatchCtx,
    channel_id: &str,
    payload_b64: &str,
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
    connection_nonce: &str,
    expires_at_ms: u64,
) {
    let Some(session) = PENDING_SESSIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(channel_id)
        .cloned()
    else {
        log::warn!("[devices/bus] no pending pairing for handshake channel_id={channel_id}");
        return;
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
    if result.frame.is_empty()
        || result.pairing_id.is_some()
        || result.session_id.is_some()
        || result.device_public_key.is_some()
    {
        log::warn!("[devices/bus] malformed TinySecurity handshake result channel_id={channel_id}");
        return;
    }
    if let Err(error) = emit_frame(channel_id, &encode_frame(&result.frame)).await {
        log::warn!("[devices/bus] handshake ACK emit failed channel_id={channel_id}: {error}");
        return;
    }
    PENDING_HANDSHAKES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(channel_id.to_owned());
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
    if !refreshed.is_ok_and(|response| response.session_id.as_deref() == Some(&active.session_id))
    {
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
    match store::insert_device(
        config,
        channel_id,
        &session.label,
        &device_public_key,
        &token_hash,
    ) {
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
                    },
                );
            PENDING_SESSIONS
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
    PENDING_HANDSHAKES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(channel_id);
    ACTIVE_SESSIONS
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
