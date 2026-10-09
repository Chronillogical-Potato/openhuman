//! The process's storage backend on the `tinystoragedrivers` ports.
//!
//! One URL picks where durable state that has moved onto the storage ports
//! lives: `OPENHUMAN_STORAGE_URL`, else `[storage] url` in `config.toml`
//! ([`crate::config::StorageConfig`]). With neither set — the desktop default —
//! nothing here is opened and every domain keeps the classic on-disk layout
//! under the workspace.
//!
//! When a URL is set, the host opens it once at startup ([`open`]) and
//! installs it ([`install`]); domains reach it through [`installed`] and bind
//! their records to an agent with [`scope_for_agent`]. The first consumer is
//! the session store: the host installs TinyAgents' `DriverSessionStores`
//! over this backend, so transcripts, turn states, records and journals live
//! in it, one scope per agent.
//!
//! Drivers are Cargo features of this crate: `storage-sqlite`,
//! `storage-mongodb`, `storage-file` (memory is always available). A URL for a
//! driver the build does not carry fails at [`open`] naming the feature, so a
//! misconfigured deployment stops at boot instead of at its first write.

use std::sync::{Arc, LazyLock, RwLock};

pub use tinystoragedrivers::{
    Scope, ScopedStorage, StorageBackend, StorageConfig as StorageUrl, StorageError,
};

use crate::config::schema::storage::redact_url;
use crate::config::Config;

/// The environment variable that overrides `[storage] url`.
pub const STORAGE_URL_VAR: &str = "OPENHUMAN_STORAGE_URL";

static BACKEND: LazyLock<RwLock<Option<Arc<dyn StorageBackend>>>> =
    LazyLock::new(|| RwLock::new(None));

/// The storage URL in effect: [`STORAGE_URL_VAR`], else `config`'s
/// `[storage] url`. Blank values count as unset. `None` keeps the classic
/// on-disk layout.
pub fn configured_url(config: &Config) -> Option<String> {
    url_from(std::env::var(STORAGE_URL_VAR).ok(), config)
}

/// [`configured_url`] with the environment read made explicit, so the rule
/// is testable without mutating process-wide state.
pub fn url_from(env: Option<String>, config: &Config) -> Option<String> {
    env.into_iter()
        .chain(config.storage.url.clone())
        .map(|url| url.trim().to_string())
        .find(|url| !url.is_empty())
}

/// Parses and opens the backend `url` names.
///
/// # Errors
///
/// An unparseable URL, a driver this build was compiled without (the error
/// names the Cargo feature), or a backend that cannot be reached.
pub async fn open(url: &str) -> Result<Arc<dyn StorageBackend>, StorageError> {
    let parsed = StorageUrl::parse(url)?;
    tracing::info!(
        target: "openhuman::storage",
        driver = parsed.driver(),
        url = %redact_url(url),
        "[storage] opening the configured backend"
    );
    tinystoragedrivers::open(&parsed).await
}

/// Makes `backend` the process's storage backend; returns the previous one.
pub fn install(backend: Arc<dyn StorageBackend>) -> Option<Arc<dyn StorageBackend>> {
    let mut slot = BACKEND
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    slot.replace(backend)
}

/// The installed backend, when the host configured one.
pub fn installed() -> Option<Arc<dyn StorageBackend>> {
    BACKEND
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// Removes the installed backend; returns whether there was one.
pub fn clear() -> bool {
    BACKEND
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
        .is_some()
}

/// The storage scope agent `agent_id`'s records live under — the same
/// mapping the session store uses, so every domain agrees on it.
pub fn scope_for_agent(agent_id: &str) -> Scope {
    tinyagents_session::DriverSessionStores::scope_for(agent_id)
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
