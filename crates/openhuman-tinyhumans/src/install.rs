//! Process-level installation: give a core booted by *any* host — the desktop
//! shell, the TUI, the CLI, a test — its TinyHumans backend connection.
//!
//! Hosts that build the core through [`crate::RuntimeBuilder`] do not need
//! this; it is for hosts that boot the core themselves
//! (`run_server_embedded_with_ready`, `run_core_from_args`, `CoreBuilder`)
//! and for test fixtures. Idempotent: calling it again re-installs an
//! equivalent transport and is harmless.

use std::sync::{Arc, Mutex, OnceLock};

use openhuman_core::api::transport::{install_backend_transport, installed_backend_transport};
use openhuman_core::api::{set_product_identity, ProductIdentity};

use crate::transport::SdkBackendTransport;

/// What [`install`] sets up.
#[derive(Debug, Default, Clone)]
pub struct InstallOptions {
    /// The `x-sdk-name` this process reports on every backend request. `None`
    /// keeps whatever identity is already set (the core's default is
    /// `"openhuman"`). Set it here, before the transport is built, because the
    /// transport captures the attribution headers once.
    pub product_identity: Option<ProductIdentity>,
}

impl InstallOptions {
    /// Report `identity` as this process's product on every backend request.
    pub fn product_identity(mut self, identity: ProductIdentity) -> Self {
        self.product_identity = Some(identity);
        self
    }
}

/// Why [`install`] could not set the process up.
#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    /// The SDK transport's HTTP client could not be built.
    #[error("failed to build the TinyHumans backend transport: {0:#}")]
    Transport(anyhow::Error),
}

static INSTALLED: OnceLock<Mutex<Option<Arc<SdkBackendTransport>>>> = OnceLock::new();

/// Install the SDK-backed backend transport as the process-global transport
/// (and optionally set the product identity first).
///
/// Returns the transport so a host that also builds the core through
/// `CoreBuilder` can bind it there explicitly with
/// `CoreBuilder::backend_transport`; binding is optional because the core
/// resolves the process global when a context carries none.
pub fn install(options: InstallOptions) -> Result<Arc<SdkBackendTransport>, InstallError> {
    let slot = INSTALLED.get_or_init(|| Mutex::new(None));
    let mut guard = slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

    if let Some(identity) = options.product_identity.clone() {
        log::debug!("[tinyhumans] install: product identity {}", identity.as_str());
        set_product_identity(identity);
        // A new identity means new attribution headers; rebuild below.
        *guard = None;
    }

    if let Some(existing) = guard.as_ref() {
        // Re-install into the core slot in case something cleared it (tests).
        if installed_backend_transport().is_none() {
            install_backend_transport(existing.clone());
        }
        log::trace!("[tinyhumans] install: already installed");
        return Ok(existing.clone());
    }

    let transport = Arc::new(SdkBackendTransport::new().map_err(InstallError::Transport)?);
    install_backend_transport(transport.clone());
    *guard = Some(transport.clone());
    log::info!("[tinyhumans] install: backend transport installed");
    Ok(transport)
}

/// Whether [`install`] has run in this process (and its transport is still
/// the core's global one).
pub fn is_installed() -> bool {
    INSTALLED
        .get()
        .and_then(|slot| {
            slot.lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_ref()
                .map(|_| installed_backend_transport().is_some())
        })
        .unwrap_or(false)
}
