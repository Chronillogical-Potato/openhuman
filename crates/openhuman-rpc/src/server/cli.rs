//! The server behind `openhuman-core run` / `serve`.

use std::future::Future;
use std::pin::Pin;

use crate::core_host::core::server_launcher::{install_server_launcher, ServeRequest};

/// Install this crate's server as the one the core CLI's `run` / `serve`
/// subcommands start. Call once, before
/// `run_core_from_args`; later calls are
/// no-ops.
pub fn install_cli_server() {
    crate::http_host::ensure_registered();
    install_server_launcher(launch);
}

/// The [`ServerLauncher`](openhuman_tinyhumans::embed::seams::ServerLauncher)
/// behind `run` / `serve`: the standalone server shims.
pub(crate) fn launch(
    request: ServeRequest,
) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>> {
    Box::pin(async move {
        log::debug!(
            "[rpc:cli] starting server host={:?} port={:?} socketio={} headless_api={} mode={}",
            request.host,
            request.port,
            request.socketio_enabled,
            request.headless_api,
            request.mode
        );
        if request.mode == crate::core_host::core::runtime::Mode::Saas {
            let config = request
                .saas_config
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("--mode saas needs --saas-config <file>"))?;
            super::run_server_saas(request.host.as_deref(), request.port, config).await
        } else if request.headless_api {
            super::run_server_headless(request.host.as_deref(), request.port).await
        } else {
            super::run_server(
                request.host.as_deref(),
                request.port,
                request.socketio_enabled,
            )
            .await
        }
    })
}
