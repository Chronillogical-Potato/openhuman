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

/// `openhuman-core run --help`.
pub const RUN_HELP: &str = "\
Usage: openhuman run [--host <addr>] [--port <u16>] [--jsonrpc-only|--headless-api]
                     [--mode single-user|saas] [--saas-config <file>] [-v|--verbose]

  --host <addr>          Bind address (default: 127.0.0.1 or OPENHUMAN_CORE_HOST)
  --port <u16>           Listen address port (default: 7788 or OPENHUMAN_CORE_PORT)
  --jsonrpc-only         HTTP JSON-RPC only; disable Socket.IO
  --headless-api         HTTP JSON-RPC only; disable all background services
  --mode <mode>          single-user (default) or saas (or OPENHUMAN_MODE=saas)
  --saas-config <file>   The operator's SaaS config; required with --mode saas
  -v, --verbose          Shorthand for RUST_LOG=debug when RUST_LOG is unset

Logging: set RUST_LOG (e.g. RUST_LOG=debug openhuman run). Default level is info.";

/// Resolve the operating mode from `--mode`, `OPENHUMAN_MODE` and
/// `--saas-config`.
///
/// The environment can only raise the mode to SaaS, never lower it: a
/// deployment that sets `OPENHUMAN_MODE=saas` cannot be talked back into
/// single-user by a flag. SaaS needs an operator config, and a config without
/// SaaS is a mistake worth refusing rather than ignoring.
pub fn resolve_mode(
    flag: Option<&str>,
    env: Option<&str>,
    saas_config: Option<std::path::PathBuf>,
) -> anyhow::Result<(crate::core::runtime::Mode, Option<std::path::PathBuf>)> {
    use crate::core::runtime::Mode;
    let parse = |raw: &str, source: &str| {
        raw.parse::<Mode>()
            .map_err(|e| anyhow::anyhow!("{source}: {e}"))
    };
    let from_flag = flag.map(|raw| parse(raw, "--mode")).transpose()?;
    let from_env = env
        .filter(|raw| !raw.trim().is_empty())
        .map(|raw| parse(raw, "OPENHUMAN_MODE"))
        .transpose()?;
    let mode = if from_env == Some(Mode::Saas) {
        Mode::Saas
    } else {
        from_flag.unwrap_or_default()
    };
    match (mode, &saas_config) {
        (Mode::Saas, None) => anyhow::bail!("--mode saas needs --saas-config <file>"),
        (Mode::SingleUser, Some(_)) => {
            anyhow::bail!("--saas-config is only meaningful with --mode saas")
        }
        _ => Ok((mode, saas_config)),
    }
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
