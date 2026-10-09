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

pub mod documents;
pub mod secrets;

use std::future::Future;
use std::sync::{Arc, LazyLock, OnceLock, RwLock};

pub use tinystoragedrivers::{
    Blocking, DocumentStore, DocumentStoreExt, MemoryStorage, Scope, ScopedStorage, StorageBackend,
    StorageConfig as StorageUrl, StorageError,
};

use crate::config::schema::storage::redact_url;
use crate::config::Config;

/// The environment variable that overrides `[storage] url`.
pub const STORAGE_URL_VAR: &str = "OPENHUMAN_STORAGE_URL";

/// A holder for one backend. The process has exactly one ([`BACKEND`]);
/// tests make their own, so they never change what other tests in the same
/// process see.
#[derive(Default)]
struct Slot(RwLock<Option<Arc<dyn StorageBackend>>>);

impl Slot {
    fn install(&self, backend: Arc<dyn StorageBackend>) -> Option<Arc<dyn StorageBackend>> {
        self.0
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .replace(backend)
    }

    fn installed(&self) -> Option<Arc<dyn StorageBackend>> {
        self.0
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn clear(&self) -> bool {
        self.0
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            .is_some()
    }
}

static BACKEND: LazyLock<Slot> = LazyLock::new(Slot::default);

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
    BACKEND.install(backend)
}

/// The installed backend, when the host configured one.
pub fn installed() -> Option<Arc<dyn StorageBackend>> {
    BACKEND.installed()
}

/// Removes the installed backend; returns whether there was one.
pub fn clear() -> bool {
    BACKEND.clear()
}

/// The storage scope of the current call: the acting agent's
/// ([`scope_for_agent`]) when the dispatch carries one
/// (`CoreContext::session_agent`), else [`Scope::local`] — except in SaaS
/// mode, where a call with no acting agent is refused rather than given a
/// bucket every user would share.
///
/// # Errors
///
/// In SaaS mode, when the current context names no agent.
pub fn current_scope() -> Result<Scope, StorageError> {
    let agent = crate::core::runtime::CoreContext::current()
        .and_then(|context| context.session_agent().map(str::to_string));
    scope_from(agent.as_deref(), crate::core::runtime::mode::is_saas())
}

/// [`current_scope`] with its two inputs made explicit, so the rule is
/// testable without a booted context or a locked mode.
///
/// # Errors
///
/// When `saas` and there is no `agent`.
pub fn scope_from(agent: Option<&str>, saas: bool) -> Result<Scope, StorageError> {
    match agent {
        Some(agent) => Ok(scope_for_agent(agent)),
        None if saas => Err(StorageError::invalid_input(
            "no acting agent in SaaS mode; refusing a shared storage scope",
        )),
        None => Ok(Scope::local()),
    }
}

/// The installed backend bound to [`current_scope`], or `None` when the host
/// configured no backend (the classic on-disk layout).
///
/// # Errors
///
/// When the scope cannot be resolved ([`current_scope`]) or the backend
/// refuses it.
pub fn current_scoped() -> Result<Option<ScopedStorage>, StorageError> {
    installed()
        .map(|backend| backend.for_scope(&current_scope()?))
        .transpose()
}

/// Runs `future` to completion from synchronous code, on one dedicated
/// runtime thread shared by every caller in the process.
///
/// For domain stores whose API is synchronous (most of the core's), so they
/// can call the async storage ports without `block_in_place` — which would
/// panic on a current-thread runtime.
///
/// # Errors
///
/// When the bridge cannot start, or `future` called back into it.
pub fn block_on<T, F>(future: F) -> Result<T, StorageError>
where
    F: Future<Output = Result<T, StorageError>> + Send + 'static,
    T: Send + 'static,
{
    static BRIDGE: OnceLock<Result<Blocking, String>> = OnceLock::new();
    let bridge = BRIDGE
        .get_or_init(|| Blocking::new().map_err(|error| error.to_string()))
        .as_ref()
        .map_err(|error| StorageError::backend(error.clone()))?;
    bridge.run(future)?
}

/// [`block_on`] for a future whose error is an `anyhow::Error` — the stores
/// tinyflows puts on the ports return those, with typed errors (such as
/// `FlowUpdateError`) a caller may downcast, so they pass through unchanged.
///
/// # Errors
///
/// The future's own error, or the bridge's when it cannot start.
pub fn block_on_anyhow<T, F>(future: F) -> anyhow::Result<T>
where
    F: Future<Output = anyhow::Result<T>> + Send + 'static,
    T: Send + 'static,
{
    block_on(async move { Ok(future.await) })?
}

/// Whether a backend with this driver name may be shared by several
/// processes at once. A MongoDB database can be; SQLite files, the memory
/// driver and plain files belong to the one process that opened them.
///
/// Boot-time recovery (interrupting in-flight turns, reaping orphaned runs)
/// is only sound on a backend no other process can be writing to.
pub fn driver_is_shared(driver: &str) -> bool {
    driver == "mongodb"
}

/// Whether the installed backend may be shared with other processes; `false`
/// when none is installed. See [`driver_is_shared`].
pub fn installed_is_shared() -> bool {
    installed().is_some_and(|backend| driver_is_shared(backend.driver()))
}

/// The storage scope agent `agent_id`'s records live under — the same
/// mapping the session store uses, so every domain agrees on it.
pub fn scope_for_agent(agent_id: &str) -> Scope {
    tinyagents_session::DriverSessionStores::scope_for(agent_id)
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
