//! The migration as a service: what the RPCs and the background job call.
//!
//! - [`scan`]: whether there is anything to move (the UI shows nothing when
//!   there is not).
//! - [`start`]: runs the migration in the background; at most one run per
//!   workspace at a time.
//! - [`status`]: its progress, with a run the app quit in the middle of
//!   reported as interrupted.
//! - [`retry`]: puts the items that could not be moved back in line.
//! - [`tick`]: memory's background job, every few minutes. It starts or
//!   resumes an automatic run, which itself waits for a free period (and,
//!   on a shared legacy tree, for the user's consent).

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex, PoisonError};

use serde::Serialize;

use super::claim;
use super::copy::legacy_present;
use super::job::LayoutHost;
use super::job::{run, Trigger};
use super::state::{self, MigrationState, Phase};
use crate::config::Config;
use crate::memory::error::{MemoryError, MemoryResult};

/// Workspaces with a migration running now.
static RUNNING: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// `memory_migration_start` params.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct StartParams {
    /// The user's consent to take a legacy tree other accounts may share.
    #[serde(default)]
    pub takeover: bool,
}

/// What [`scan`] found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScanView {
    /// There is legacy memory still to move.
    pub needed: bool,
    /// The legacy tree may be shared with other accounts, so moving it needs
    /// the user's consent.
    pub shared: bool,
}

/// [`status`]'s answer.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MigrationStatus {
    /// The saved progress.
    pub state: MigrationState,
    /// A run is going now.
    pub running: bool,
    /// The app stopped in the middle of a run; it resumes on its own, or the
    /// user starts it again.
    pub interrupted: bool,
}

/// Whether `config`'s account has legacy memory to move.
///
/// # Errors
///
/// When the state cannot be read, the engines cannot be bound, or the legacy
/// tree cannot be read.
pub async fn scan(config: &Config, host: &dyn LayoutHost) -> MemoryResult<ScanView> {
    let state = state::load(&config.workspace_dir)?;
    let claim = match host.legacy_claim(config) {
        // Memory off or signed out: nothing to show.
        Err(MemoryError::Off(_)) => None,
        claim => claim?,
    };
    let shared = claim.is_some();
    let needed = match state.phase {
        Phase::Cleaned => false,
        // Another account on this machine took the shared tree.
        Phase::Idle if claim.as_ref().map(claim::held_by_other).transpose()? == Some(true) => false,
        Phase::Idle if !state.switched => match host.engines(config) {
            // Memory off or signed out: nothing to show.
            Err(MemoryError::Off(_)) => false,
            Err(error) => return Err(error),
            Ok(engines) => legacy_present(&*engines.legacy).await?,
        },
        _ => true,
    };
    Ok(ScanView { needed, shared })
}

/// The migration's progress for `config`'s workspace.
///
/// # Errors
///
/// When the state file cannot be read.
pub fn status(config: &Config) -> MemoryResult<MigrationStatus> {
    let state = state::load(&config.workspace_dir)?;
    let running = is_running(config);
    let interrupted = !running && matches!(state.phase, Phase::Copying | Phase::Cleaning);
    Ok(MigrationStatus {
        state,
        running,
        interrupted,
    })
}

pub(crate) fn is_running(config: &Config) -> bool {
    RUNNING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .contains(&config.workspace_dir)
}

/// Releases the workspace's run slot when the run ends, however it ends.
struct RunGuard(PathBuf);

impl Drop for RunGuard {
    fn drop(&mut self) {
        RUNNING
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.0);
    }
}

/// Starts a run in the background. Returns `false`, starting nothing, when
/// one is already running for this workspace, or while an import of old
/// local memory is unfinished (import first, then reorganise).
pub fn start(
    config: Config,
    host: Arc<dyn LayoutHost>,
    trigger: Trigger,
    paused: Arc<dyn Fn() -> bool + Send + Sync>,
) -> bool {
    let _gate = crate::memory::import::START_GATE
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if crate::memory::import::in_progress(&config) {
        tracing::info!("[memory:layout_migration] waiting for the import to finish");
        return false;
    }
    let workspace = config.workspace_dir.clone();
    if !RUNNING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(workspace.clone())
    {
        return false;
    }
    let guard = RunGuard(workspace);
    crate::core::runtime::spawn_scoped(async move {
        let _guard = guard;
        let outcome = run(&config, host.as_ref(), trigger, || {
            let paused = paused();
            async move { paused }
        })
        .await;
        match outcome {
            Ok(outcome) => {
                tracing::info!(?outcome, "[memory:layout_migration] run ended");
            }
            Err(error) => {
                tracing::warn!(code = error.code(), "[memory:layout_migration] run failed");
                record_failure(&config, &error);
            }
        }
    });
    true
}

/// Saves why a run stopped on an error, so the status shows it.
fn record_failure(config: &Config, error: &MemoryError) {
    if let Ok(mut state) = state::load(&config.workspace_dir) {
        state.phase = Phase::Paused;
        state.error = Some(error.to_string());
        if let Err(save) = state::save(&config.workspace_dir, &state) {
            tracing::warn!(
                code = save.code(),
                "[memory:layout_migration] state not saved"
            );
        }
    }
}

/// Puts the items that could not be moved back in line: the next run copies
/// again from the start (what was copied replays) and cleans up again.
///
/// # Errors
///
/// When the state cannot be read or saved, or a run is going now.
pub fn retry(config: &Config) -> MemoryResult<MigrationState> {
    if is_running(config) {
        return Err(MemoryError::invalid("the migration is running"));
    }
    let mut state = state::load(&config.workspace_dir)?;
    state.failures.clear();
    state.incomplete.clear();
    state.cursor = None;
    state.error = None;
    if state.switched {
        state.caught_up = false;
        state.cleaning = false;
    }
    if state.phase != Phase::Idle {
        state.phase = Phase::Paused;
    }
    state::save(&config.workspace_dir, &state)?;
    Ok(state)
}

/// Memory's background job: starts or resumes an automatic run unless the
/// migration is done or already running. Returns whether one started.
pub fn tick(
    config: &Config,
    host: Arc<dyn LayoutHost>,
    paused: Arc<dyn Fn() -> bool + Send + Sync>,
) -> bool {
    match state::load(&config.workspace_dir) {
        Ok(state)
            if state.phase == Phase::Cleaned
                && (state.rechecked
                    || !crate::memory::import::listed_unconfirmed(&config.workspace_dir)) =>
        {
            false
        }
        Ok(_) if paused() || is_running(config) => false,
        Ok(_) => start(config.clone(), host, Trigger::Auto, paused),
        Err(error) => {
            tracing::warn!(
                code = error.code(),
                "[memory:layout_migration] state unreadable"
            );
            false
        }
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
