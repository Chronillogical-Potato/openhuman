//! The on-disk session store the app, the CLI and the TUI install.
//!
//! [`SqliteSessionStores`] is a
//! [`SessionStoreProvider`](tinyagents_session::port::SessionStoreProvider)
//! over the layout OpenHuman has always written under its workspace:
//!
//! ```text
//! {workspace}/session_raw/*.jsonl                         transcripts
//! {workspace}/session_db/sessions.db                      run ledger (SQLite)
//! {workspace}/memory/conversations/turn_states/…          turn snapshots
//! {workspace}/tinyagents_store/{kv,journal}/              run status, goals,
//!                                                         todos, turn journal
//! ```
//!
//! The desktop app, the CLI and the TUI install it ([`install`]). The core
//! and the embed facade reach session state through the port, and keep a
//! fallback to the same files only for hosts that install no store.
//!
//! It serves one operator: every agent shares the workspace, as it always
//! has, so it does not claim the per-agent isolation a multi-user host's
//! store must provide.
//!
//! The workspace is resolved on every call rather than fixed at
//! construction, because the desktop rebinds it when a different user signs
//! in.
//!
//! ```
//! use std::sync::Arc;
//! use openhuman_rpc::session_store::SqliteSessionStores;
//! use tinyagents_session::port::SessionStoreProvider;
//!
//! let dir = tempfile::tempdir().unwrap();
//! let stores = SqliteSessionStores::at(dir.path());
//! assert_eq!(stores.workspace_dir().as_deref(), Some(dir.path()));
//! let agent = stores.for_agent("orchestrator");
//! assert!(agent.transcripts.root_for_thread("no-such-thread").is_none());
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tinyagents_session::port::{AgentStores, SessionStoreProvider};
use tinyagents_session::transcript::import::ops::open_session_stores;
use tinyagents_session::transcript::FileTranscriptLocator;
use tinyagents_session::turn_state::TurnStateStore;

/// Resolves the workspace the stores live in, at the moment they are needed.
type WorkspaceResolver = dyn Fn() -> PathBuf + Send + Sync;

/// OpenHuman's on-disk session layout as a session store provider.
#[derive(Clone)]
pub struct SqliteSessionStores {
    workspace: Arc<WorkspaceResolver>,
}

impl std::fmt::Debug for SqliteSessionStores {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteSessionStores")
            .field("workspace", &(self.workspace)())
            .finish()
    }
}

impl SqliteSessionStores {
    /// Stores under the fixed `workspace` directory.
    pub fn at(workspace: impl Into<PathBuf>) -> Self {
        let workspace = workspace.into();
        Self::resolving(move || workspace.clone())
    }

    /// Stores under whichever workspace `resolve` names when asked — for a
    /// host whose workspace can change while it runs.
    pub fn resolving(resolve: impl Fn() -> PathBuf + Send + Sync + 'static) -> Self {
        Self {
            workspace: Arc::new(resolve),
        }
    }

    fn current(&self) -> PathBuf {
        (self.workspace)()
    }
}

impl SessionStoreProvider for SqliteSessionStores {
    /// Every agent shares the workspace: the single-operator layout.
    fn for_agent(&self, agent_id: &str) -> AgentStores {
        let workspace = self.current();
        log::trace!(
            "[rpc:session_store] stores for agent={agent_id} workspace={}",
            workspace.display()
        );
        stores_at(&workspace)
    }

    /// Marks turns left in flight by an unclean shutdown interrupted, and
    /// settles run-ledger rows a dead process left running — the two sweeps
    /// the core always ran at boot.
    fn recover(&self) -> anyhow::Result<()> {
        let workspace = self.current();
        let now = chrono::Utc::now().to_rfc3339();
        let turns =
            tinyagents_session::turn_state::store::mark_all_interrupted(workspace.clone(), &now)
                .map_err(anyhow::Error::msg)?;
        let runs = tinyagents_session::run_ledger::interrupt_orphaned_agent_runs(&workspace)?;
        log::info!(
            "[rpc:session_store] recovered workspace={} interrupted_turns={turns} settled_runs={runs}",
            workspace.display()
        );
        Ok(())
    }

    fn destination_key(&self) -> Option<String> {
        Some(self.current().to_string_lossy().into_owned())
    }

    fn workspace_dir(&self) -> Option<PathBuf> {
        Some(self.current())
    }
}

/// The stores the layout keeps under `workspace`.
fn stores_at(workspace: &Path) -> AgentStores {
    let kv_and_journal = open_session_stores(workspace);
    AgentStores {
        transcripts: Arc::new(FileTranscriptLocator::new(workspace)),
        turn_states: Arc::new(TurnStateStore::new(workspace.to_path_buf())),
        kv: Arc::new(kv_and_journal.kv),
        journal: Arc::new(kv_and_journal.journal),
    }
}

/// Installs [`SqliteSessionStores`] over the current workspace as the
/// process's session store. Call it before the core boots, so its recovery
/// sweep runs; the workspace is resolved per call because the desktop rebinds
/// it when a different user signs in.
pub fn install() {
    openhuman_core::agent::session_store::install(Arc::new(SqliteSessionStores::resolving(
        openhuman_core::agent::session_store::context_workspace_dir,
    )));
}

/// Installs the session store the host's configuration asks for, before the
/// core boots.
///
/// With no storage URL (`OPENHUMAN_STORAGE_URL`, else `[storage] url`) this is
/// [`install`]: the classic on-disk layout, unchanged. With one, the backend is
/// opened, made the process's storage backend
/// ([`openhuman_core::storage::install`]), and TinyAgents'
/// `DriverSessionStores` is installed over it: every agent's transcripts, turn
/// states, records and journal in that backend, one storage scope per agent.
///
/// A single-process backend (SQLite, memory, files) also interrupts an
/// agent's in-flight turns the first time it is opened, since only an earlier
/// process can have left them. MongoDB does not: several processes may share
/// that database, and another one may own those turns.
///
/// # Errors
///
/// When a URL is configured but cannot be parsed or opened. A deployment that
/// asked for a backend must not quietly fall back to local files.
pub async fn install_for_host() -> anyhow::Result<()> {
    use anyhow::Context as _;

    let url = match std::env::var(openhuman_core::storage::STORAGE_URL_VAR) {
        Ok(url) if !url.trim().is_empty() => Some(url.trim().to_string()),
        _ => match openhuman_core::config::ops::loader::load_config_with_timeout().await {
            Ok(config) => openhuman_core::storage::configured_url(&config),
            Err(error) => {
                tracing::warn!(
                    "[session_store] config unavailable ({error}); keeping the on-disk layout"
                );
                None
            }
        },
    };
    let Some(url) = url else {
        install();
        return Ok(());
    };
    let backend = openhuman_core::storage::open(&url)
        .await
        .context("opening the configured storage backend")?;
    openhuman_core::storage::install(Arc::clone(&backend));
    let single_process = backend.driver() != "mongodb";
    let provider = tinyagents_session::DriverSessionStores::new(backend)
        .context("starting the session store bridge")?
        .recover_on_open(single_process);
    tracing::info!(
        target: "openhuman_rpc::session_store",
        recover_on_open = single_process,
        "[session_store] installed the storage-backed session store"
    );
    openhuman_core::agent::session_store::install(Arc::new(provider));
    Ok(())
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
