//! Event bus handlers for the devices domain.
//!
//! Subscribes to `tunnel:peer-status` and `tunnel:frame` events published by
//! `socket::event_handlers` and drives:
//! - Updating `PEER_STATUS` in `rpc.rs`.
//! - Completing the X25519 handshake when the device sends its pubkey.
//! - Persisting the `PairedDevice` record after a successful handshake.
//! - Publishing `DomainEvent::DevicePaired / DevicePeerOnline / DevicePeerOffline`.

use crate::core::runtime::DispatchCtx;
use std::sync::{Arc, OnceLock};

use crate::core::bus::BUS;
use crate::core::events::DomainEvent;
use crate::security::devices::crypto::{base64url_encode, DeviceKeypair, TunnelCipher};
use crate::security::devices::rpc::{
    ACTIVE_CIPHERS, PEER_STATUS, PENDING_KEYPAIRS, PENDING_SESSIONS,
};
use crate::security::devices::store;
use crate::security::devices::tunnel_client::emit_frame;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tinybus::EventHandler;
use tinybus::SubscriptionHandle;
use tinysecurity_crypto::device::{KeyPair, PublicKey, Role, Tunnel};

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
                handle_peer_offline(channel_id);
            }
            DomainEvent::DeviceTunnelFrame {
                channel_id,
                payload_b64,
            } => {
                // Handle the frame as the agent the device belongs to, so its
                // pairing record and the RPCs it sends land in that agent's
                // scope (`super::owner`).
                let pending = PENDING_SESSIONS
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get(channel_id.as_str())
                    .cloned();
                // Tunnel frames arrive over the host's socket: its own tenant,
                // and in SaaS none (the frame is then dropped below).
                let ctx = DispatchCtx::host_default();
                match super::owner::owner_of(&ctx, channel_id, pending.as_ref()).await {
                    Ok(owner) => {
                        let handled = crate::storage::agents::within_agent(
                            &ctx,
                            owner.as_deref(),
                            handle_tunnel_frame(&ctx, channel_id, payload_b64),
                        )
                        .await;
                        // Fail closed: no context can act for the owner.
                        if handled.is_none() {
                            log::warn!(
                                "[devices/bus] dropping tunnel frame channel_id={channel_id}: \
                                 its owner has no context to act under"
                            );
                        }
                    }
                    // Fail closed: an unknown owner must not become `local`.
                    Err(failed) => log::warn!(
                        "[devices/bus] dropping tunnel frame channel_id={channel_id}: owner \
                         lookup failed in scope={} ({})",
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

#[derive(Debug)]
struct HandshakePayload {
    device_pubkey: String,
    client_ephemeral_pubkey: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JsonHandshakePayload {
    device_pubkey: String,
    client_ephemeral_pubkey: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct TunnelEnvelope {
    #[serde(rename = "requestId")]
    request_id: String,
    kind: String,
    seq: u64,
    payload: Value,
}

#[derive(Debug, Deserialize)]
struct TunnelRpcPayload {
    method: String,
    #[serde(default)]
    params: Value,
}

async fn handle_peer_online(channel_id: &str) {
    log::info!("[devices/bus] peer online channel_id={}", channel_id);
    super::lock_unpoisoned(&PEER_STATUS)
        .insert(channel_id.to_string(), true);
    // No re-publish: the event was already published by socket::event_handlers.
}

fn handle_peer_offline(channel_id: &str) {
    log::info!("[devices/bus] peer offline channel_id={}", channel_id);
    super::lock_unpoisoned(&PEER_STATUS)
        .insert(channel_id.to_string(), false);
    // No re-publish: the event was already published by socket::event_handlers.
}

/// Handle an incoming `tunnel:frame` — first frame from the device contains its
/// X25519 public key sealed to the core's public key. After successful decryption
/// we derive the shared secret and persist the `PairedDevice`.
async fn handle_tunnel_frame(ctx: &DispatchCtx, channel_id: &str, payload_b64: &str) {
    log::debug!(
        "[devices/bus] tunnel:frame channel_id={} payload_len={}",
        channel_id,
        payload_b64.len()
    );

    // Decode the outer base64url envelope.
    let frame_bytes = match crate::security::devices::crypto::base64url_decode(payload_b64) {
        Ok(b) => b,
        Err(e) => {
            log::warn!(
                "[devices/bus] bad base64url in tunnel:frame channel_id={}: {e}",
                channel_id
            );
            return;
        }
    };

    if frame_bytes.first() == Some(&0x02) {
        handle_encrypted_rpc_frame(channel_id, &frame_bytes).await;
        return;
    }

    // Wire format for the handshake frame:
    //
    //   0x03 || eph_pub(32) || nonce(24) || ciphertext+tag
    //
    // Version byte 0x03 = "sealed-handshake". The device generates an ephemeral
    // X25519 keypair, performs DH with corePubkey, then seals its static pubkey
    // (32 bytes) with XChaCha20-Poly1305. The core decrypts using the same
    // ephemeral DH to recover the device's static public key, then performs a
    // second DH (core_static ⟷ device_static) for the session key.
    //
    // Version byte 0x02 = "encrypted-frame" (used post-handshake, handled later).
    //
    // Old raw-DH handshakes and plaintext pubkeys are rejected. The peer must
    // upgrade its tunnel implementation before connecting again.
    if frame_bytes.first() != Some(&HANDSHAKE_VERSION) {
        log::warn!(
            "[devices/bus] unsupported handshake version channel_id={}",
            channel_id
        );
        return;
    }
    // Claim the session and its keypair before any crypto, cipher installation
    // or ACK. The claim is single-use, and an expired or malformed expiry fails
    // closed. This also binds every handshake to the channel that created it.
    let (keypair, session, existing_device) =
        if let Some((keypair, session)) =
            take_pending_pairing_for_handshake(channel_id, chrono::Utc::now())
        {
            (keypair, Some(session), None)
        } else {
            match recover_existing_pairing(ctx, channel_id).await {
                Ok(Some((keypair, device))) => (Arc::new(keypair), None, Some(device)),
                Ok(None) => {
                    log::warn!(
                        "[devices/bus] no live pairing identity for channel_id={channel_id}"
                    );
                    return;
                }
                Err(error) => {
                    log::warn!("[devices/bus] cannot recover pairing {channel_id}: {error}");
                    return;
                }
            }
        };

    let (_, plaintext) = match keypair.open_handshake(&frame_bytes) {
        Ok(opened) => opened,
        Err(error) => {
            log::warn!("[devices/bus] sealed-handshake rejected channel_id={channel_id}: {error}");
            return;
        }
    };
    let handshake_payload = match serde_json::from_slice::<JsonHandshakePayload>(&plaintext) {
        Ok(payload) => payload,
        Err(error) => {
            log::warn!("[devices/bus] malformed handshake payload channel_id={channel_id}: {error}");
            return;
        }
    };
    let device_pubkey_b64 = handshake_payload.device_pubkey;

    if let Some(device) = &existing_device
        && !matches_paired_device_key(device, &device_pubkey_b64)
    {
        log::warn!(
            "[devices/bus] restart handshake key mismatch for channel_id={channel_id}"
        );
        return;
    }

    log::info!(
        "[devices/bus] handshake frame received channel_id={} device_pubkey_len={}",
        channel_id,
        device_pubkey_b64.len()
    );

    let Some(client_eph_pubkey) = handshake_payload.client_ephemeral_pubkey.as_deref() else {
        log::warn!(
            "[devices/bus] handshake missing client ephemeral key channel_id={}",
            channel_id
        );
        return;
    };
    if let Err(error) = install_v2_cipher_and_ack(
        channel_id,
        &keypair,
        &device_pubkey_b64,
        client_eph_pubkey,
    )
    .await
    {
        log::error!("[devices/bus] v2 handshake ack failed channel_id={channel_id}: {error}");
        return;
    }

    if let Some(session) = session {
        let label = session.channel_id.clone();
        let session_token_hash = hash_session_token(&session.pairing_token);
        match ctx.load_config().await {
            Ok(config) => match store::insert_device_with_key(
                &config,
                channel_id,
                &label,
                &device_pubkey_b64,
                &session_token_hash,
                &session.core_private_key_encrypted,
            ) {
                Ok(device) => {
                    super::owner::remember(
                        channel_id,
                        ctx.tenant().ok().and_then(|tenant| tenant.agent),
                    );
                    log::info!(
                        "[devices/bus] device persisted channel_id={} label={}",
                        device.channel_id,
                        device.label
                    );
                    BUS.publish(DomainEvent::DevicePaired {
                        channel_id: channel_id.to_string(),
                        device_pubkey: device_pubkey_b64,
                        label: Some(label),
                    });
                }
                Err(error) => {
                    log::error!("[devices/bus] failed to persist device {channel_id}: {error}");
                    abandon_unpersisted_pairing(channel_id);
                }
            },
            Err(error) => {
                log::warn!("[devices/bus] could not load config to persist device {channel_id}: {error}");
                abandon_unpersisted_pairing(channel_id);
            }
        }
    } else if let Some(device) = existing_device {
        log::info!(
            "[devices/bus] restored tunnel for paired device channel_id={} label={}",
            device.channel_id,
            device.label
        );
    }
}

/// Drops the pairing session and session cipher of a handshake whose device
/// could not be persisted: with no row in the store, later frames would
/// otherwise keep being accepted for a device nothing records
/// (`super::owner` treats a missing row as the handshake persistence window).
pub(super) fn abandon_unpersisted_pairing(channel_id: &str) {
    super::lock_unpoisoned(&PENDING_SESSIONS).remove(channel_id);
    super::lock_unpoisoned(&ACTIVE_CIPHERS).remove(channel_id);
    log::warn!("[devices/bus] pairing abandoned channel_id={channel_id}: device not persisted");
}

fn take_pending_pairing_for_handshake(
    channel_id: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<(
    std::sync::Arc<crate::security::devices::crypto::DeviceKeypair>,
    super::types::PairingSession,
)> {
    let session = super::lock_unpoisoned(&PENDING_SESSIONS).remove(channel_id);
    let keypair = super::lock_unpoisoned(&PENDING_KEYPAIRS).remove(channel_id);
    let (Some(session), Some(keypair)) = (session, keypair) else {
        return None;
    };
    if session.channel_id != channel_id {
        return None;
    }
    let expires_at = chrono::DateTime::parse_from_rfc3339(&session.expires_at).ok()?;
    (expires_at.with_timezone(&chrono::Utc) > now).then_some((keypair, session))
}

fn restore_paired_keypair(
    encrypted_private_key: &str,
    secret_store: &super::super::secret_storage::SecretStore,
) -> Result<crate::security::devices::crypto::DeviceKeypair, String> {
    let plaintext = zeroize::Zeroizing::new(
        secret_store
            .decrypt(encrypted_private_key)
            .map_err(|error| format!("[devices/bus] decrypt persisted pairing key: {error}"))?,
    );
    let encoded = zeroize::Zeroizing::new(
        crate::security::devices::crypto::base64url_decode(&plaintext)
            .map_err(|error| format!("[devices/bus] decode persisted pairing key: {error}"))?,
    );
    let bytes: [u8; 32] = encoded
        .as_slice()
        .try_into()
        .map_err(|_| "[devices/bus] persisted pairing key must be 32 bytes".to_string())?;
    crate::security::devices::crypto::DeviceKeypair::from_private_bytes(
        zeroize::Zeroizing::new(bytes),
    )
}

async fn recover_existing_pairing(
    ctx: &DispatchCtx,
    channel_id: &str,
) -> Result<Option<(DeviceKeypair, super::types::PairedDevice)>, String> {
    let config = ctx.load_config().await.map_err(|error| error.to_string())?;
    let Some(device) = store::get_device(&config, channel_id).map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };
    let secret_store = super::rpc::build_secret_store(&config);
    let Some(keypair) = restore_live_pairing_key(&device, &secret_store)? else {
        return Ok(None);
    };
    Ok(Some((keypair, device)))
}

fn restore_live_pairing_key(
    device: &super::types::PairedDevice,
    secret_store: &super::super::secret_storage::SecretStore,
) -> Result<Option<DeviceKeypair>, String> {
    if device.revoked {
        return Ok(None);
    }
    let Some(encrypted_key) = device.core_private_key_encrypted.as_deref() else {
        // Rows created before durable pairing identities were added cannot
        // safely reconstruct the core key and require a new pairing.
        return Ok(None);
    };
    restore_paired_keypair(encrypted_key, secret_store).map(Some)
}

fn matches_paired_device_key(device: &super::types::PairedDevice, handshake_key: &str) -> bool {
    !device.revoked && device.device_pubkey == handshake_key
}

async fn install_v2_cipher_and_ack(
    channel_id: &str,
    core_keypair: &DeviceKeypair,
    device_public_key: &str,
    client_ephemeral_key: &str,
) -> Result<(), String> {
    let device_public = PublicKey::decode(device_public_key)
        .map_err(|error| format!("[devices/bus] invalid device public key: {error}"))?;
    let client_ephemeral = PublicKey::decode(client_ephemeral_key)
        .map_err(|error| format!("[devices/bus] invalid client ephemeral key: {error}"))?;
    let server_ephemeral = KeyPair::generate()
        .map_err(|error| format!("[devices/bus] generate server ephemeral key: {error}"))?;
    let server_public = server_ephemeral.public_key();
    let keys = core_keypair
        .session_keys(device_public, &server_ephemeral, client_ephemeral, server_public)
        .map_err(|error| format!("[devices/bus] derive tunnel keys: {error}"))?;
    let tunnel = Tunnel::new(Role::Server, keys);
    let ack = serde_json::to_vec(&json!({
        "kind": "handshake_ack",
        "server_ephemeral_pubkey": server_public.encode(),
    }))
    .map_err(|error| format!("[devices/bus] handshake ack serialize failed: {error}"))?;
    let ack_frame = core_keypair
        .seal_ack(client_ephemeral, &ack)
        .map_err(|error| format!("[devices/bus] handshake ack seal failed: {error}"))?;

    super::lock_unpoisoned(&ACTIVE_CIPHERS).insert(
        channel_id.to_string(),
        Arc::new(std::sync::Mutex::new(TunnelCipher::from_native(tunnel))),
    );
    emit_frame(channel_id, &base64url_encode(&ack_frame)).await?;
    log::info!(
        "[devices/bus] v2 tunnel cipher installed channel_id={} server_eph_len=32",
        channel_id
    );
    Ok(())
}

async fn handle_encrypted_rpc_frame(channel_id: &str, frame_bytes: &[u8]) {
    let cipher = {
        let map = super::lock_unpoisoned(&ACTIVE_CIPHERS);
        map.get(channel_id).cloned()
    };
    let Some(cipher) = cipher else {
        log::warn!(
            "[devices/bus] encrypted frame with no active cipher channel_id={}",
            channel_id
        );
        return;
    };

    let plaintext = {
        let mut guard = super::lock_unpoisoned(&cipher);
        match guard.open(frame_bytes) {
            Ok(bytes) => bytes,
            Err(e) => {
                log::warn!(
                    "[devices/bus] encrypted frame open failed channel_id={}: {e}",
                    channel_id
                );
                return;
            }
        }
    };

    let envelope = match serde_json::from_slice::<TunnelEnvelope>(&plaintext) {
        Ok(envelope) => envelope,
        Err(e) => {
            log::warn!(
                "[devices/bus] tunnel envelope parse failed channel_id={}: {e}",
                channel_id
            );
            return;
        }
    };

    if envelope.kind != "request" {
        log::debug!(
            "[devices/bus] ignoring non-request tunnel envelope kind={} channel_id={}",
            envelope.kind,
            channel_id
        );
        return;
    }

    let request = match serde_json::from_value::<TunnelRpcPayload>(envelope.payload) {
        Ok(request) => request,
        Err(e) => {
            emit_tunnel_error(
                channel_id,
                &cipher,
                &envelope.request_id,
                format!("invalid tunnel RPC payload: {e}"),
            )
            .await;
            return;
        }
    };

    log::debug!(
        "[devices/bus] tunnel RPC request channel_id={} method={} request_id={}",
        channel_id,
        request.method,
        envelope.request_id
    );

    let result = crate::core::invoke::invoke_method(
        crate::core::runtime::DispatchCtx::process_default(),
        crate::core::invoke::default_state(),
        &request.method,
        request.params,
    )
    .await;

    match result {
        Ok(value) => {
            emit_tunnel_response(channel_id, &cipher, &envelope.request_id, "response", value)
                .await;
        }
        Err(message) => {
            emit_tunnel_response(
                channel_id,
                &cipher,
                &envelope.request_id,
                "error",
                Value::String(message),
            )
            .await;
        }
    }
}

async fn emit_tunnel_error(
    channel_id: &str,
    cipher: &Arc<std::sync::Mutex<TunnelCipher>>,
    request_id: &str,
    message: String,
) {
    emit_tunnel_response(
        channel_id,
        cipher,
        request_id,
        "error",
        Value::String(message),
    )
    .await;
}

async fn emit_tunnel_response(
    channel_id: &str,
    cipher: &Arc<std::sync::Mutex<TunnelCipher>>,
    request_id: &str,
    kind: &str,
    payload: Value,
) {
    let response = TunnelEnvelope {
        request_id: request_id.to_string(),
        kind: kind.to_string(),
        seq: 0,
        payload,
    };
    let plaintext = match serde_json::to_vec(&response) {
        Ok(bytes) => bytes,
        Err(e) => {
            log::error!(
                "[devices/bus] tunnel response serialize failed channel_id={}: {e}",
                channel_id
            );
            return;
        }
    };
    let encrypted = {
        let guard = super::lock_unpoisoned(cipher);
        match guard.seal(&plaintext) {
            Ok(frame) => frame,
            Err(e) => {
                log::error!(
                    "[devices/bus] tunnel response seal failed channel_id={}: {e}",
                    channel_id
                );
                return;
            }
        }
    };
    let payload_b64 = base64url_encode(&encrypted);
    if let Err(e) = emit_frame(channel_id, &payload_b64).await {
        log::error!(
            "[devices/bus] tunnel response emit failed channel_id={}: {e}",
            channel_id
        );
    }
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
