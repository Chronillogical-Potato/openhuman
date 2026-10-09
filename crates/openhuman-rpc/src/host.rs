//! The shared host boot: one entry per host shape, so each host's `main`
//! becomes a few lines instead of a hand-assembled sequence.
//!
//! | Entry | Replaces | Builder |
//! |---|---|---|
//! | [`cli`] | `tinyhumans::install` → `server::install_cli_server` → `run_core_from_args` | [`cli_builder`]: the `cli` preset, connected, with the server launcher and the `http_host` controllers |
//! | [`desktop`] | `tinyhumans::install` + `server::run_server_embedded_with_ready` | [`desktop_builder`]: the `desktop` preset, connected, with the bearer, listener, services, server launcher and `http_host` controllers |
//! | [`tui`] | `tinyhumans::install` + `session_store::install_for_host` + `CoreBuilder(full, none)` | [`tui_builder`]: the `tui` preset, connected, with the on-disk session store ([`tui`] swaps in the configured storage URL's store) |
//!
//! Each `*_builder` returns a [`tinyhumans::RuntimeBuilder`] so a host can
//! adjust it (product identity, hooks, a different ranker) before handing it
//! to the matching `serve_*` / `build` call.
//!
//! "Connected" means [`tinyhumans::RuntimeBuilder::connect`]: the SDK
//! transport (process global and bound to the runtime), the hosted RPC
//! proxies and, with the `jev` feature, the Jev `tool_search` ranker.
//!
//! # Behavior notes
//!
//! - The desktop and CLI servers install the on-disk session store for the
//!   life of the process (see `server::shims::build_and_serve`), exactly as the
//!   `run_server*` shims do; the TUI hands it to the builder, which restores
//!   the previous provider when its runtime drops at exit. Both honour a
//!   storage URL (`OPENHUMAN_STORAGE_URL`, else `[storage] url`): with one,
//!   conversations live in that backend instead of the on-disk layout.
//! - Builder seams follow embed's install/restore rules: the hosted and
//!   `http_host` controllers and the server launcher stay for the process; the
//!   Jev ranker is restored when the runtime drops. A desktop server that
//!   restarts in place re-installs it on the next build.
//! - The runtime does not start background services itself; [`desktop`]
//!   starts them through `server::serve`, as the shims always have.
//!
//! [`tinyhumans::RuntimeBuilder`]: openhuman_tinyhumans::RuntimeBuilder
//! [`tinyhumans::RuntimeBuilder::connect`]: openhuman_tinyhumans::RuntimeBuilder::connect

#[cfg(feature = "server")]
use std::sync::Arc;

#[cfg(feature = "server")]
use openhuman_tinyhumans::embed::{ServiceSet, TokenSource};
use openhuman_tinyhumans::RuntimeBuilder;
#[cfg(feature = "server")]
use tokio_util::sync::CancellationToken;

#[cfg(feature = "server")]
pub use crate::server::EmbeddedReadySignal;

/// The CLI host's builder: the `cli` preset with this crate's server as the
/// `run` / `serve` launcher and the `http_host.*` controllers registered.
#[cfg(feature = "server")]
pub fn cli_builder() -> RuntimeBuilder {
    RuntimeBuilder::cli()
        .server_launcher(crate::server::cli::launch)
        .controller_extension(crate::http_host::extension())
}

/// Run the core's command-line dispatcher on `args` (without the binary
/// name) as the `openhuman-core` binary does: connected to the TinyHumans
/// backend, with this crate's server behind `run` / `serve`.
///
/// # Errors
///
/// The transport could not be built, a controller extension was refused, or
/// the dispatched command failed.
#[cfg(feature = "server")]
pub fn cli(args: &[String]) -> anyhow::Result<()> {
    log::debug!(
        "[rpc:host] cli command={} argc={}",
        args.first().map(String::as_str).unwrap_or("<none>"),
        args.len()
    );
    cli_builder().run_from_args(args)
}

/// What the embedded desktop server binds and how it authenticates.
#[cfg(feature = "server")]
#[derive(Clone)]
pub struct DesktopOptions {
    /// Bind host; `None` falls back to `OPENHUMAN_CORE_HOST`, then loopback.
    pub host: Option<String>,
    /// Preferred port; `None` falls back to `OPENHUMAN_CORE_PORT`, then 7788.
    /// A stale listener of our own is taken over; see `server::serve`.
    pub port: Option<u16>,
    /// Serve Socket.IO alongside HTTP JSON-RPC (default `true`).
    pub socketio: bool,
    /// The per-launch bearer, handed over in memory. `None` keeps the
    /// env-or-file token.
    pub rpc_token: Option<Arc<String>>,
}

#[cfg(feature = "server")]
impl Default for DesktopOptions {
    fn default() -> Self {
        Self {
            host: None,
            port: None,
            socketio: true,
            rpc_token: None,
        }
    }
}

#[cfg(feature = "server")]
impl std::fmt::Debug for DesktopOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the bearer.
        f.debug_struct("DesktopOptions")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("socketio", &self.socketio)
            .field("rpc_token", &self.rpc_token.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

/// The desktop host's builder: the `desktop` preset with every background
/// service (Socket.IO per `options`), the in-memory bearer and listener, this
/// crate's server launcher and the `http_host.*` controllers.
#[cfg(feature = "server")]
pub fn desktop_builder(options: &DesktopOptions) -> RuntimeBuilder {
    let mut services = ServiceSet::desktop();
    services.socketio = options.socketio;
    let mut builder = RuntimeBuilder::desktop()
        .services(services)
        .server_launcher(crate::server::cli::launch)
        .controller_extension(crate::http_host::extension());
    if let Some(token) = options.rpc_token.clone() {
        builder = builder.token(TokenSource::Fixed(token));
    }
    if let Some(host) = options.host.clone() {
        builder = builder.listen_host(host);
    }
    if let Some(port) = options.port {
        builder = builder.listen_port(port);
    }
    builder
}

/// Boot the embedded desktop core and serve it until `shutdown_token` is
/// cancelled: [`desktop_builder`], connected, then [`serve_desktop`].
///
/// # Errors
///
/// The transport or runtime could not be built, or the listener failed.
#[cfg(feature = "server")]
pub async fn desktop(
    options: DesktopOptions,
    shutdown_token: CancellationToken,
    ready_tx: tokio::sync::oneshot::Sender<EmbeddedReadySignal>,
) -> anyhow::Result<()> {
    log::debug!("[rpc:host] desktop options={options:?}");
    serve_desktop(desktop_builder(&options), shutdown_token, ready_tx).await
}

/// Connect `builder` and serve it as the embedded desktop core: the session
/// store installed, the runtime built, background services started, the
/// listener bound (taking over a stale listener of our own on the preferred
/// port, else falling back), `ready_tx` signalled with the bound port, and
/// the server run until `shutdown_token` is cancelled.
///
/// # Errors
///
/// The transport or runtime could not be built, or the listener failed.
#[cfg(feature = "server")]
pub async fn serve_desktop(
    builder: RuntimeBuilder,
    shutdown_token: CancellationToken,
    ready_tx: tokio::sync::oneshot::Sender<EmbeddedReadySignal>,
) -> anyhow::Result<()> {
    let builder = builder.connect().map_err(|error| {
        log::warn!("[rpc:host] desktop: TinyHumans connection failed: {error}");
        anyhow::Error::new(error)
    })?;
    crate::server::shims::build_and_serve(builder, Some(ready_tx), Some(shutdown_token)).await
}

/// The terminal UI's builder: the `tui` preset (every domain, no transport,
/// no background services) with the on-disk session store.
#[cfg(feature = "session-store")]
pub fn tui_builder() -> RuntimeBuilder {
    RuntimeBuilder::tui().session_store(crate::session_store::provider())
}

/// Boot the TUI's in-process core, connected. The returned runtime owns the
/// core for the session; `Runtime::core_runtime` hands the TUI its
/// `CoreRuntime`.
///
/// Conversations stay in the classic on-disk layout, as the desktop keeps
/// them, unless a storage URL (`OPENHUMAN_STORAGE_URL` / `[storage] url`) is
/// set: then [`tui_builder`]'s on-disk store is replaced with the storage-backed
/// one ([`crate::session_store::provider_for_host`]).
///
/// # Errors
///
/// A configured storage URL could not be opened, or the transport or runtime
/// could not be built.
#[cfg(feature = "session-store")]
pub async fn tui() -> anyhow::Result<openhuman_tinyhumans::embed::Runtime> {
    log::debug!("[rpc:host] tui: building connected runtime");
    let session_store = crate::session_store::provider_for_host().await?;
    let runtime = tui_builder()
        .session_store(session_store)
        .build()
        .await
        .map_err(|error| {
            log::warn!("[rpc:host] tui: runtime build failed: {error}");
            anyhow::Error::new(error)
        })?;
    log::info!("[rpc:host] tui: core built (DomainSet::full, ServiceSet::none)");
    Ok(runtime)
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
