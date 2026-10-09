//! The CLI entry: [`RuntimeBuilder::run_from_args`] and [`run_from_args`].
//!
//! `openhuman_core::run_core_from_args` dispatches a whole command line —
//! `run`/`serve`, `call`, `agent`, `mcp`, the namespace commands — and each
//! subcommand boots the core its own way (the server through the installed
//! [`server_launcher`](RuntimeBuilder::server_launcher), the others
//! in-process). So this is a thin wrapper rather than a builder-driven boot:
//! it installs the builder's process-global pieces for the life of the
//! process, then hands the arguments to the core. The builder's runtime
//! knobs (domains, services, workspace, token, listen) do not apply; a
//! server launcher reads its own preset.

use std::sync::Arc;

use super::{RuntimeBuilder, RuntimeError};

impl RuntimeBuilder {
    /// Install this builder's process-global pieces, then run the core's
    /// command-line dispatcher on `args` (without the binary name).
    ///
    /// Installed for the rest of the process, never restored: the backend
    /// transport (as the process global), the memory engine, the session
    /// store, controller extensions, the server launcher, the tool ranker and
    /// the embedder hooks. A [`live_policy`](Self::live_policy) is ignored
    /// with a warning — each subcommand's boot installs its own.
    ///
    /// # Errors
    ///
    /// A refused controller extension, or whatever the dispatched command
    /// returns.
    pub fn run_from_args(mut self, args: &[String]) -> anyhow::Result<()> {
        let command = args.first().map(String::as_str).unwrap_or("<none>");
        log::debug!(
            "[embed][cli] run_from_args command={command} argc={}",
            args.len()
        );

        if let Some(transport) = self.backend_transport.take() {
            openhuman_core::backend::install_backend_transport(transport);
            log::debug!("[embed][cli] backend transport installed (process global)");
        }
        if let Some(engine) = self.memory_engine.take() {
            openhuman_core::memory::engine::install_host_engine(engine);
        }
        if let Some(provider) = self.session_store.take() {
            let _previous = openhuman_core::agent::session_store::install(Arc::clone(&provider));
            log::debug!("[embed][cli] session store installed (process lifetime)");
        }
        if self.seams.live_policy.take().is_some() {
            log::warn!(
                "[embed][cli] live_policy is ignored by run_from_args; \
                 each subcommand installs the policy its config describes"
            );
        }
        let seams = std::mem::take(&mut self.seams)
            .install()
            .map_err(|error| anyhow::Error::new(RuntimeError::Invalid(error)))?;
        seams.persist();

        openhuman_core::run_core_from_args(args)
    }
}

/// Run the core's command-line dispatcher with the [`RuntimeBuilder::cli`]
/// preset — no backend transport, extensions or launcher. Hosts that need
/// those (the TinyHumans transport, the JSON-RPC server) configure a builder
/// and call [`RuntimeBuilder::run_from_args`] instead.
pub fn run_from_args(args: &[String]) -> anyhow::Result<()> {
    RuntimeBuilder::cli().run_from_args(args)
}
