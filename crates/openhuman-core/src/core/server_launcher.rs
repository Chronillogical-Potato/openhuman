//! The port through which `openhuman-core run` / `serve` starts a server.
//!
//! The JSON-RPC server lives in `openhuman-rpc`, which sits above this crate,
//! so the CLI cannot call it directly. A host binary installs a launcher once
//! at startup (`openhuman_rpc::server::install_cli_server()`) before it hands
//! its arguments to [`run_core_from_args`](crate::run_core_from_args).
//! Without one, `run` / `serve` fail with an error that says so instead of
//! starting a core nothing can reach.

use std::future::Future;
use std::pin::Pin;
use std::sync::OnceLock;

/// What the `run` / `serve` subcommand asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServeRequest {
    /// Bind host, when the operator passed one.
    pub host: Option<String>,
    /// Bind port, when the operator passed one.
    pub port: Option<u16>,
    /// Serve Socket.IO alongside HTTP JSON-RPC (`--jsonrpc-only` clears it).
    pub socketio_enabled: bool,
    /// Request/response API only, with no background services.
    pub headless_api: bool,
    /// The operating mode (`--mode`).
    pub mode: crate::core::runtime::Mode,
    /// The operator's SaaS config file (`--saas-config`); required in SaaS mode.
    pub saas_config: Option<std::path::PathBuf>,
}

/// Starts a server for a [`ServeRequest`] and resolves when it stops.
pub type ServerLauncher =
    fn(ServeRequest) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>;

static LAUNCHER: OnceLock<ServerLauncher> = OnceLock::new();

/// Install the launcher `run` / `serve` use. The first call wins; later calls
/// are ignored, so every host path can install without coordinating.
pub fn install_server_launcher(launcher: ServerLauncher) {
    if LAUNCHER.set(launcher).is_err() {
        log::debug!("[cli] server launcher already installed; keeping the first");
    }
}

/// The installed launcher, if a host installed one.
pub fn installed_server_launcher() -> Option<ServerLauncher> {
    LAUNCHER.get().copied()
}

#[cfg(test)]
#[path = "server_launcher_tests.rs"]
mod tests;
