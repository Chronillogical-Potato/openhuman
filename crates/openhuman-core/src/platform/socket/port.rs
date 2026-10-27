//! Host-owned Socket.IO transport port.
//!
//! The core owns the socket RPC contract and consumes this interface; the
//! TinyHumans host installs the SDK-backed implementation.

use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use parking_lot::RwLock;

use super::models::{ConnectionStatus, SocketState};

pub type TokenProvider =
    Arc<dyn Fn() -> futures::future::BoxFuture<'static, Result<String, String>> + Send + Sync>;

#[async_trait]
pub trait SocketPort: Send + Sync + 'static {
    async fn connect(&self, url: &str, token: &str) -> Result<(), String>;
    async fn connect_with_provider(
        &self,
        url: &str,
        provider: TokenProvider,
    ) -> Result<(), String>;
    async fn disconnect(&self) -> Result<(), String>;
    async fn emit(&self, event: &str, data: serde_json::Value) -> Result<(), String>;
    async fn emit_with_ack(
        &self,
        event: &str,
        data: serde_json::Value,
        timeout: std::time::Duration,
    ) -> Result<serde_json::Value, String>;

    fn get_state(&self) -> SocketState;
    fn is_connected(&self) -> bool;
    fn is_live_for(&self, url: &str, token: &str) -> bool;
    fn status_for(&self, url: &str, token: &str) -> Option<ConnectionStatus>;
    fn is_loop_active(&self) -> bool;
    fn loop_stopped_on_failure(&self) -> bool;

    async fn lock_identity_rebind(&self) -> SocketIdentityRebindGuard {
        let lock = IDENTITY_REBIND.get_or_init(|| Arc::new(tokio::sync::Mutex::new(())));
        SocketIdentityRebindGuard(lock.clone().lock_owned().await)
    }
}

/// Keeps concurrent identity-sensitive reconnect transactions serialized.
pub struct SocketIdentityRebindGuard(tokio::sync::OwnedMutexGuard<()>);

static IDENTITY_REBIND: OnceLock<Arc<tokio::sync::Mutex<()>>> = OnceLock::new();
static SOCKET_PORT: OnceLock<RwLock<Option<Arc<dyn SocketPort>>>> = OnceLock::new();

fn slot() -> &'static RwLock<Option<Arc<dyn SocketPort>>> {
    SOCKET_PORT.get_or_init(|| RwLock::new(None))
}

/// Install the hosted socket port. Reinstallation replaces the previous host.
pub fn set_global_socket_port(port: Arc<dyn SocketPort>) {
    *slot().write() = Some(port);
}

/// Return the currently installed hosted socket port.
pub fn global_socket_port() -> Option<Arc<dyn SocketPort>> {
    slot().read().clone()
}

/// Compatibility name for core consumers while the concrete transport is host-owned.
pub fn global_socket_manager() -> Option<Arc<dyn SocketPort>> {
    global_socket_port()
}

/// Compatibility registration name used by host installation code.
pub fn set_global_socket_manager(port: Arc<dyn SocketPort>) {
    set_global_socket_port(port);
}

#[cfg(test)]
#[path = "port_tests.rs"]
mod tests;
