//! Retrying the items a finished import skipped because the engine refused
//! them (`memory_import_retry_failed`), and resuming such a retry after an
//! interruption. A retry is the user's action: when it stops it says so and
//! waits for Retry again.

use std::collections::HashSet;
use std::path::Path;

use tinymemory_integrations::cortex::is_insufficient_credits;
use tinymemory_integrations::import::LegacyWorkspace;

use super::{
    legacy_id, read_file, skips_item, status, with_retries, write_file, FailedItem, ImportFile,
    PauseCheck, RUNNING,
};
use crate::config::Config;
use crate::memory::engine::{self, BoundEngine};
use crate::memory::error::{MemoryError, MemoryResult};
use crate::memory::types::{ImportPhase, ImportState};

/// `memory_import_retry_failed`: stores again the items a finished import
/// skipped because the engine refused them. Items that now store leave the
/// list; ones refused again stay, with the new reason.
pub async fn retry_failed(config: &Config) -> MemoryResult<ImportState> {
    let file = read_file(&config.workspace_dir);
    // A retry the app quit during is still a retry the user may press
    // again; a live one is answered with its status by `begin_retry`.
    let interrupted_retry = file.state.phase == ImportPhase::Running && file.retrying;
    if !(file.state.phase == ImportPhase::Done || interrupted_retry) || file.failed.is_empty() {
        return Err(MemoryError::invalid(
            "no failed items to retry: the import has not finished or skipped nothing",
        ));
    }
    begin_retry(config, file, None)
}

/// Starts (or, after an interruption, resumes) a retry of `file.failed`. A
/// resume the background job started passes its `paused` check, asked
/// before every item, as the import asks it before every batch.
pub(super) fn begin_retry(
    config: &Config,
    mut file: ImportFile,
    paused: Option<PauseCheck>,
) -> MemoryResult<ImportState> {
    let bound = engine::resolve(config).engine()?;
    let workspace_dir = config.workspace_dir.clone();
    let claimed = RUNNING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(workspace_dir.clone());
    if !claimed {
        return Ok(status(config));
    }
    file.state.phase = ImportPhase::Running;
    file.state.error = None;
    file.retrying = true;
    write_file(&workspace_dir, &file);
    let state = file.state.clone();
    tracing::info!(
        failed = file.failed.len(),
        "[memory:import] retrying failed items"
    );
    tokio::spawn(async move {
        retry_run(&workspace_dir, &bound, file, paused).await;
        RUNNING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&workspace_dir);
    });
    Ok(state)
}

/// Why a retry of failed items stopped, and that pressing Retry again
/// continues it: a retry is the user's action, so nothing resumes it alone.
fn retry_stop_message(error: tinymemory_api::Error) -> String {
    if error.is_transient() {
        return "the memory service is unavailable; press Retry again once it is back".to_string();
    }
    if is_insufficient_credits(&error) {
        return "not enough credits to retry these items; top up, then press Retry again"
            .to_string();
    }
    format!(
        "the retry stopped: {}; press Retry again",
        MemoryError::from(error)
    )
}

async fn retry_run(
    workspace_dir: &Path,
    bound: &BoundEngine,
    mut file: ImportFile,
    paused: Option<PauseCheck>,
) {
    // An item with no legacy id cannot be found again, so it is never
    // matched (every item the importer yields has one).
    let wanted: HashSet<String> = file
        .failed
        .iter()
        .map(|failed| failed.id.clone())
        .filter(|id| !id.is_empty())
        .collect();
    let reader_dir = workspace_dir.to_path_buf();
    let items = tokio::task::spawn_blocking(move || {
        let workspace = LegacyWorkspace::open(&reader_dir).map_err(|error| error.to_string())?;
        workspace
            .items()
            .filter_map(|imported| match imported {
                Ok(imported) if wanted.contains(&legacy_id(&imported.item)) => {
                    Some(Ok(imported.item))
                }
                Ok(_) => None,
                Err(error) => Some(Err(error.to_string())),
            })
            .collect::<Result<Vec<_>, String>>()
    })
    .await
    .map_err(|error| error.to_string())
    .and_then(|items| items);
    let mut failure = None;
    match items {
        Err(error) => {
            failure = Some(format!(
                "reading the legacy store failed: {error}; press Retry again"
            ));
        }
        Ok(items) => {
            for item in items {
                if paused.as_ref().is_some_and(|paused| paused()) {
                    // Left a running retry with what is still listed: the
                    // next unpaused tick resumes it.
                    tracing::info!("[memory:import] background paused; retry left to resume");
                    write_file(workspace_dir, &file);
                    return;
                }
                let id = legacy_id(&item);
                match with_retries(|| bound.engine.store(item.clone())).await {
                    Ok(_) => {
                        file.failed.retain(|failed| failed.id != id);
                        file.state.imported += 1;
                        file.state.failed = file.failed.len() as u64;
                        // Persisted per item, so a quit mid-retry keeps what
                        // was stored off the list instead of sending it again.
                        write_file(workspace_dir, &file);
                    }
                    Err(error) if skips_item(&error) => {
                        let again = FailedItem::new(&item, error);
                        if let Some(failed) = file.failed.iter_mut().find(|f| f.id == id) {
                            *failed = again;
                        }
                        // Persisted as it changes, like a stored item.
                        write_file(workspace_dir, &file);
                    }
                    Err(error) => {
                        failure = Some(retry_stop_message(error));
                        break;
                    }
                }
            }
        }
    }
    file.state.failed = file.failed.len() as u64;
    // The import itself is finished either way, so it stays `Done`: a retry
    // stopped by the engine or the account keeps the items still to retry
    // and says why, and the user retries again once that is resolved.
    file.state.phase = ImportPhase::Done;
    file.state.error = failure;
    file.retrying = false;
    tracing::info!(
        still_failed = file.failed.len(),
        "[memory:import] retry of failed items finished"
    );
    write_file(workspace_dir, &file);
}
