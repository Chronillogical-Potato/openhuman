//! OpenHuman on the hosted TinyHumans backend.
//!
//! Layering, bottom up:
//!
//! - `openhuman-core` runs agents, memory, tools and RPC and knows the backend
//!   only through a port, [`BackendTransport`]. It has no SDK dependency and
//!   runs without any TinyHumans connection.
//! - `openhuman-embed` is the library facade over the core.
//! - **this crate** implements the port with the vendored `tinyhumans-sdk`
//!   ([`SdkBackendTransport`]), installs it into a process ([`install`]),
//!   offers a [`RuntimeBuilder`] that boots an embed runtime already
//!   connected, registers the hosted-backend RPC proxies ([`hosted`]:
//!   billing, team, referral, announcements) with the core's controller
//!   registry, and owns the host-side **login and backend session**
//!   ([`session`]: [`SessionManager`], [`SessionClient`], [`CoreLink`]) — the
//!   core only ever *takes* a credential.
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
//! [`RuntimeBuilder`] is the configuration path: it hands the embed builder
//! the SDK transport, the hosted controllers and the Jev ranker through
//! embed's seam options, and forwards the embed presets and knobs. Hosts that
//! still boot the core themselves (the desktop shell's
//! `run_server_embedded_with_ready`, the CLI's `run_core_from_args`, a
//! `CoreBuilder`) call [`install`] once before the first backend-touching
//! dispatch instead; it resolves the same wiring and applies it to the
//! process globals.
//!
//! This crate depends on `openhuman-embed` alone. Core internals it needs
//! come through embed's `#[doc(hidden)] __host` list; it re-exports only its
//! own curated surface, never the core.

pub use openhuman_embed as embed;

pub mod backend;
pub mod hosted;
mod install;
#[cfg(feature = "jev")]
pub mod jev;
pub mod jwt;
mod runtime;
pub mod session;
pub mod transport;

pub use backend::{product_identity, set_product_identity, ProductIdentity};
pub use hosted::extension as hosted_controllers;
pub use install::{install, is_installed, InstallError, InstallOptions};
pub use openhuman_embed::{
    BackendRequest, BackendTransport, BackendTransportError, TransportProfile,
};
pub use runtime::{RuntimeBuilder, RuntimeError};
pub use session::{
    cache::{CachedUser, CurrentUserCache},
    client::{ClientHeaders, FetchMeError, SessionClient, SessionClientError},
    credential::{
        decode_jwt_exp, jwt_is_live, user_id_from_jwt_claims, user_id_from_profile_payload,
        Credential, CredentialKind,
    },
    identity,
    link::{self, CoreAuthState, CoreLink},
    manager::{SessionError, SessionEvent, SessionManager, SessionState},
};
pub use transport::{map_sdk_error, SdkBackendTransport};
