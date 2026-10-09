//! Server entry points kept for the hosts that predate the shared host boot
//! ([`crate::host`]).
//!
//! Each `run_server*` function is a thin shim: it starts from the embed host
//! preset that matches the caller ([`RuntimeBuilder::desktop`] for the
//! embedded desktop core, [`RuntimeBuilder::cli`] for a standalone server),
//! applies the caller's services, bearer and listener, then builds the
//! runtime and [`serve`](super::serve::serve)s it.
//!
//! The presets carry exactly what these shims used to hand `CoreBuilder` by
//! hand: every domain family, the `embedded_core` → `HostKind` mapping
//! (embedded == Tauri shell; standalone splits CLI / Docker via
//! `HostKind::detect_standalone`), the `OPENHUMAN_E2E` tool-group switch, and
//! no supplied config (the core discovers the operator's install).
//!
//! None of these connect the TinyHumans backend: the hosts calling them
//! install it themselves (`openhuman_tinyhumans::install`). [`crate::host`]
//! is the entry that connects it.

use std::sync::Arc;

use openhuman_tinyhumans::embed::{RuntimeBuilder, ServiceSet, TokenSource};
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
    let services = ServiceSet::headless_api();
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
    let config = crate::core_host::core::runtime::SaasConfig::load(saas_config)?;
    crate::session_store::install();
    let runtime =
        crate::core_host::core::runtime::saas::build(config, host.map(str::to_owned), port).await?;
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
/// via `openhuman::core::auth::init_rpc_token_with_value` without ever
/// reading `OPENHUMAN_CORE_TOKEN` from the process environment.  Passing
/// `None` preserves the env-as-config fallback (CLI / docker / cloud).
pub async fn run_server_embedded_with_ready(
    host: Option<&str>,
    port: Option<u16>,
    socketio_enabled: bool,
    shutdown_token: CancellationToken,
    ready_tx: tokio::sync::oneshot::Sender<EmbeddedReadySignal>,
    rpc_token: Option<Arc<String>>,
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
    rpc_token: Option<Arc<String>>,
) -> anyhow::Result<()> {
    let mut services = ServiceSet::desktop();
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
    services: ServiceSet,
    embedded_core: bool,
    shutdown_token: Option<CancellationToken>,
    ready_tx: Option<tokio::sync::oneshot::Sender<EmbeddedReadySignal>>,
    rpc_token: Option<Arc<String>>,
) -> anyhow::Result<()> {
    let preset = if embedded_core {
        RuntimeBuilder::desktop()
    } else {
        RuntimeBuilder::cli()
    };
    let builder = server_builder(preset, services, host, port, rpc_token);
    build_and_serve(builder, ready_tx, shutdown_token).await
}

/// Apply a server's services, bearer and listener to a host preset.
///
/// `rpc_token` is the in-memory bearer handoff ([`TokenSource::Fixed`]);
/// `None` keeps the preset's env-or-file token. An unset host or port is left
/// to [`serve`](super::serve::serve), which falls back to
/// `OPENHUMAN_CORE_HOST` / `OPENHUMAN_CORE_PORT` and then the defaults.
pub(crate) fn server_builder(
    preset: RuntimeBuilder,
    services: ServiceSet,
    host: Option<&str>,
    port: Option<u16>,
    rpc_token: Option<Arc<String>>,
) -> RuntimeBuilder {
    let mut builder = preset.services(services);
    // The browser E2E harness scripts direct tool calls through its mock
    // model. Keep production's fail-closed packed default, while making those
    // calls visible in the deterministic test core.
    if std::env::var_os("OPENHUMAN_E2E").is_some() {
        log::debug!("[rpc:server] OPENHUMAN_E2E set; advertising every tool group");
        builder = builder.tool_groups(openhuman_tinyhumans::embed::ToolGroups::advertised());
    }
    if let Some(token) = rpc_token {
        builder = builder.token(TokenSource::Fixed(token));
    }
    if let Some(host) = host {
        builder = builder.listen_host(host);
    }
    if let Some(port) = port {
        builder = builder.listen_port(port);
    }
    builder
}

/// Build `builder`'s runtime and serve it until shutdown.
///
/// The on-disk session store is installed for the life of the process, as the
/// shims always did, rather than through `RuntimeBuilder::session_store`
/// (which restores the previous provider when the runtime drops). The desktop
/// restarts its in-process server in place; a detached turn still writing
/// across that gap must keep landing in the classic layout, not in the
/// core's no-provider fallback.
pub(crate) async fn build_and_serve(
    builder: RuntimeBuilder,
    ready_tx: Option<tokio::sync::oneshot::Sender<EmbeddedReadySignal>>,
    shutdown_token: Option<CancellationToken>,
) -> anyhow::Result<()> {
    let summary = builder.summary();
    log::debug!(
        "[rpc:server] building runtime host_kind={:?} services={:?} fixed_token={} \
         listen_host={:?} listen_port={:?} embedded_ready={}",
        summary.host_kind,
        summary.services,
        summary.fixed_token,
        summary.listen_host,
        summary.listen_port,
        ready_tx.is_some()
    );

    // The desktop app and the CLI keep conversations in the classic on-disk
    // layout unless a storage URL is configured; the core itself carries no
    // storage. Installed before boot so its recovery sweep runs.
    crate::session_store::install_for_host().await?;
    let runtime = builder.build().await.map_err(|error| {
        log::warn!("[rpc:server] runtime build failed: {error}");
        anyhow::Error::new(error)
    })?;
    let served = super::serve::serve(runtime.core_runtime(), ready_tx, shutdown_token).await;
    // Dropping the runtime releases the process's embed runtime slot, so the
    // desktop can restart its server in place.
    drop(runtime);
    log::debug!("[rpc:server] runtime released ok={}", served.is_ok());
    served
}

#[cfg(test)]
#[path = "shims_tests.rs"]
mod tests;
