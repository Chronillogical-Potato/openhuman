//! Core-owned semantic port for the hosted device tunnel client.
//!
//! The host owns Socket.IO event names, payloads, ACK parsing, and connection
//! details. Core device operations use this typed interface and consume
//! semantic peer/frame events only.

use std::sync::{Arc, OnceLock};

use async_trait::async_trait;

/// Registration details needed by the device pairing domain after the host
/// parses the backend's `tunnel:register` ACK.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelRegistration {
    pub channel_id: String,
    pub pairing_token: String,
    /// ISO 8601 timestamp regardless of the backend's wire representation.
    pub pairing_expires_at: String,
}

/// Host implementation of the hosted device tunnel transport.
#[async_trait]
pub trait TunnelClientPort: Send + Sync + 'static {
    async fn register(&self) -> Result<TunnelRegistration, String>;
    async fn connect(&self, channel_id: &str) -> Result<(), String>;
    async fn emit_frame(&self, channel_id: &str, payload_b64: &str) -> Result<(), String>;
}

static TUNNEL_CLIENT_PORT: OnceLock<Arc<dyn TunnelClientPort>> = OnceLock::new();

/// Install the host's tunnel client. Duplicate installation leaves the first
/// process-wide adapter in place, matching the other core host ports.
pub fn set_global_tunnel_client_port(port: Arc<dyn TunnelClientPort>) {
    if TUNNEL_CLIENT_PORT.set(port).is_err() {
        log::warn!("[devices/tunnel] tunnel client port already installed; ignoring duplicate");
    }
}

/// Return the host's registered tunnel client, if one is installed.
pub fn global_tunnel_client_port() -> Option<Arc<dyn TunnelClientPort>> {
    TUNNEL_CLIENT_PORT.get().cloned()
}

fn require_port() -> Result<Arc<dyn TunnelClientPort>, String> {
    global_tunnel_client_port()
        .ok_or_else(|| "[devices/tunnel] TunnelClientPort not initialized".to_string())
}

/// Register a pairing channel through the host-owned backend tunnel.
pub async fn emit_register() -> Result<TunnelRegistration, String> {
    require_port()?.register().await
}

/// Ask the host to begin listening on a channel as the core role.
pub async fn emit_connect(channel_id: &str) -> Result<(), String> {
    require_port()?.connect(channel_id).await
}

/// Send an encrypted device frame through the host-owned backend tunnel.
pub async fn emit_frame(channel_id: &str, payload_b64: &str) -> Result<(), String> {
    require_port()?.emit_frame(channel_id, payload_b64).await
}

/// Publish a parsed peer-presence update from the host adapter.
pub fn publish_peer_status(channel_id: String, online: bool) {
    let event = if online {
        crate::core::events::DomainEvent::DevicePeerOnline { channel_id }
    } else {
        crate::core::events::DomainEvent::DevicePeerOffline { channel_id }
    };
    crate::core::bus::BUS.publish(event);
}

/// Publish a parsed encrypted frame from the host adapter.
pub fn publish_tunnel_frame(channel_id: String, payload_b64: String) {
    crate::core::bus::BUS.publish(crate::core::events::DomainEvent::DeviceTunnelFrame {
        channel_id,
        payload_b64,
    });
}
