//! OpenHuman on the hosted TinyHumans backend.
//!
//! Layering, bottom up:
//!
//! - `openhuman-core` runs agents, memory, tools and RPC and knows the backend
//!   only through a port, [`BackendTransport`]. It has no SDK dependency and
//!   runs without any TinyHumans connection.
//! - `openhuman-embed` is the library facade over the core.
//! - **this crate** implements the port with the vendored `tinyhumans-sdk`
//!   ([`SdkBackendTransport`]), installs it into a process ([`install`]) and
//!   offers a [`RuntimeBuilder`] that boots an embed runtime already
//!   connected.
//!
//! ```no_run
//! # async fn demo() -> anyhow::Result<()> {
//! use openhuman_tinyhumans::{embed::Workspace, RuntimeBuilder};
//!
//! let runtime = RuntimeBuilder::new()
//!     .workspace(Workspace::Ephemeral)
//!     .api_key("th_...")
//!     .build()
//!     .await?;
//! # let _ = runtime;
//! # Ok(())
//! # }
//! ```
//!
//! Hosts that boot the core themselves (the desktop shell's
//! `run_server_embedded_with_ready`, the CLI's `run_core_from_args`, a
//! `CoreBuilder`) call [`install`] once before the first backend-touching
//! dispatch instead.

pub use openhuman_embed as embed;

mod install;
pub mod jwt;
mod runtime;
pub mod transport;

pub use install::{install, is_installed, InstallError, InstallOptions};
pub use openhuman_embed::{
    BackendRequest, BackendTransport, BackendTransportError, TransportProfile,
};
pub use runtime::{RuntimeBuilder, RuntimeError};
pub use transport::{map_sdk_error, SdkBackendTransport};
