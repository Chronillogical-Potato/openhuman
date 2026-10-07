//! Server entry points kept for the hosts that predate
//! [`CoreBuilder`](openhuman_core::core::runtime::CoreBuilder).
//!
//! Each `run_server*` function is a thin shim that composes a `CoreBuilder`
//! and calls [`serve`](super::serve::serve).

use tokio_util::sync::CancellationToken;

use super::serve::EmbeddedReadySignal;

/// Resolves the port for the core server from environment variables or defaults.
pub(crate) fn core_port() -> u16 {
    std::env::var("OPENHUMAN_CORE_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(7788)
}

/// Resolves the bind address host for the core server from environment variables or defaults.
pub(crate) fn core_host() -> String {
    std::env::var("OPENHUMAN_CORE_HOST")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "127.0.0.1".to_string())
}

/// Runs the HTTP/JSON-RPC server.
///
/// This function binds to the specified host and port, initializes the router,
/// bootstraps long-lived runtime infrastructure, and starts serving requests.
pub async fn run_server(
    host: Option<&str>,
    port: Option<u16>,
    socketio_enabled: bool,
) -> anyhow::Result<()> {
    run_server_inner(host, port, socketio_enabled, false, None, None, None).await
}

/// Runs the request/response-only HTTP API without detached background jobs.
pub async fn run_server_headless(host: Option<&str>, port: Option<u16>) -> anyhow::Result<()> {
    let services = openhuman_core::core::runtime::ServiceSet::headless_api();
    run_server_with_services(host, port, services, false, None, None, None).await
}

/// Runs a SaaS core: many users behind a trusted gateway, booted from the
/// operator's config file and refused unless its boot guard passes.
///
/// The on-disk session store is installed before boot. It resolves the
/// workspace of the context each call runs under, so every user agent keeps
/// its sessions, transcripts and turn states in its own workspace.
pub async fn run_server_saas(
    host: Option<&str>,
    port: Option<u16>,
    saas_config: &std::path::Path,
) -> anyhow::Result<()> {
    let config = openhuman_core::core::runtime::SaasConfig::load(saas_config)?;
    crate::session_store::install();
    let runtime =
        openhuman_core::core::runtime::saas::build(config, host.map(str::to_owned), port).await?;
    super::serve::serve(&runtime, None, None).await
}

/// Like [`run_server`] but marks the instance as embedded.
pub async fn run_server_embedded(
    host: Option<&str>,
    port: Option<u16>,
    socketio_enabled: bool,
    shutdown_token: CancellationToken,
) -> anyhow::Result<()> {
    run_server_inner(
        host,
        port,
        socketio_enabled,
        true,
        Some(shutdown_token),
        None,
        None,
    )
    .await
}

/// Embedded entrypoint with an explicit readiness callback.
///
/// When the caller already holds the per-launch RPC bearer in memory (the
/// Tauri shell now that the core runs in-process — PR #1061), it should
/// pass `Some(token)` so the embedded server can seed its auth subsystem
/// via `openhuman_core::core::auth::init_rpc_token_with_value` without ever
/// reading `OPENHUMAN_CORE_TOKEN` from the process environment.  Passing
/// `None` preserves the env-as-config fallback (CLI / docker / cloud).
pub async fn run_server_embedded_with_ready(
    host: Option<&str>,
    port: Option<u16>,
    socketio_enabled: bool,
    shutdown_token: CancellationToken,
    ready_tx: tokio::sync::oneshot::Sender<EmbeddedReadySignal>,
    rpc_token: Option<std::sync::Arc<String>>,
) -> anyhow::Result<()> {
    run_server_inner(
        host,
        port,
        socketio_enabled,
        true,
        Some(shutdown_token),
        Some(ready_tx),
        rpc_token,
    )
    .await
}

/// Internal server entrypoint.
async fn run_server_inner(
    host: Option<&str>,
    port: Option<u16>,
    socketio_enabled: bool,
    embedded_core: bool,
    shutdown_token: Option<CancellationToken>,
    ready_tx: Option<tokio::sync::oneshot::Sender<EmbeddedReadySignal>>,
    rpc_token: Option<std::sync::Arc<String>>,
) -> anyhow::Result<()> {
    let mut services = openhuman_core::core::runtime::ServiceSet::desktop();
    services.socketio = socketio_enabled;
    run_server_with_services(
        host,
        port,
        services,
        embedded_core,
        shutdown_token,
        ready_tx,
        rpc_token,
    )
    .await
}

async fn run_server_with_services(
    host: Option<&str>,
    port: Option<u16>,
    services: openhuman_core::core::runtime::ServiceSet,
    embedded_core: bool,
    shutdown_token: Option<CancellationToken>,
    ready_tx: Option<tokio::sync::oneshot::Sender<EmbeddedReadySignal>>,
    rpc_token: Option<std::sync::Arc<String>>,
) -> anyhow::Result<()> {
    // `run_server_inner` is now a thin shim over the CoreBuilder/CoreRuntime
    // composition (Phase 1). It reproduces the legacy behavior exactly: all
    // background services on (`ServiceSet::desktop`), Socket.IO per the caller
    // flag, and the legacy `embedded_core` → `HostKind` mapping (embedded ==
    // Tauri shell; standalone splits CLI / Docker via `detect_standalone`).
    // See the pluggable-core work (`core::runtime`).
    let host_kind = if embedded_core {
        openhuman_core::core::types::HostKind::TauriShell
    } else {
        openhuman_core::core::types::HostKind::detect_standalone()
    };
    let token = match rpc_token {
        Some(token) => openhuman_core::core::runtime::TokenSource::Fixed(token),
        None => openhuman_core::core::runtime::TokenSource::EnvOrFile,
    };
    let mut builder = openhuman_core::core::runtime::CoreBuilder::new(host_kind)
        .token(token)
        .services(services);
    // The browser E2E harness scripts direct tool calls through its mock model.
    // Keep production's fail-closed packed default, while making those calls
    // visible in the deterministic test core without changing library hosts.
    builder = apply_e2e_tool_groups(builder);
    if let Some(host) = host {
        builder = builder.host(host);
    }
    if let Some(port) = port {
        builder = builder.port(port);
    }

    // The desktop app and the CLI keep conversations in the classic on-disk
    // layout; the core itself carries no storage. Installed before boot so
    // its recovery sweep runs.
    crate::session_store::install();
    let runtime = builder.build().await?;
    super::serve::serve(&runtime, ready_tx, shutdown_token).await
}

fn apply_e2e_tool_groups(
    builder: openhuman_core::core::runtime::CoreBuilder,
) -> openhuman_core::core::runtime::CoreBuilder {
    if std::env::var_os("OPENHUMAN_E2E").is_some() {
        builder.tool_groups(openhuman_core::tools::toolpacks::ToolGroups::advertised())
    } else {
        builder
    }
}

#[cfg(test)]
#[path = "shims_tests.rs"]
mod tests;
