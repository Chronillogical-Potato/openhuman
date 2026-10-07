//! Importing a v1 memory store into the selected engine.
//!
//! [`scan`] opens `<workspace>/memory/memory.db` read-only through
//! `tinymemory-import` and counts what it would import. [`start`] refuses to
//! run without explicit consent (importing uploads local data to the engine),
//! then runs in the background: a blocking reader walks the legacy store and
//! hands each item to the async side, which scrubs and stores it and advances
//! the resumable [`Checkpoint`]. Progress and the checkpoint persist in
//! `<workspace>/memory/import_state.json`, so a restarted import resumes where
//! the last one stopped (an item stored but not yet checkpointed is re-sent,
//! and the engine treats it as a replay). A batch the engine cannot take for
//! a transient reason (unreachable, overloaded, its indexer behind) is retried
//! with backoff; if the engine stays unavailable the run stops and, like an
//! import the app quit in the middle of, resumes on its own
//! ([`resume_interrupted`]). One stopped by a failure that is not about a
//! single item and will not pass by itself (credits exhausted, signed out)
//! waits for the user to start it again.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use serde::{Deserialize, Serialize};
use tinymemory_api::ItemKind;
use tinymemory_integrations::cortex::is_insufficient_credits;
use tinymemory_integrations::import::{Checkpoint, ImportedItem, LegacyWorkspace};

use crate::config::Config;

use super::engine::{self, BoundEngine};
use super::error::{MemoryError, MemoryResult};
use super::types::{ImportCounts, ImportPhase, ImportScanView, ImportState};

/// Items stored between two checkpoint writes.
const CHECKPOINT_EVERY: u64 = 25;

/// Items per bulk store (`MemoryEngine::store_many`).
const STORE_BATCH: usize = 25;

/// How long to wait before each retry of a batch the engine could not take
/// for a transient reason (unreachable, overloaded, indexer behind: an
/// `Unavailable` error). After the last, the run stops and is resumed by the
/// background job.
#[cfg(not(test))]
const RETRY_DELAYS: [std::time::Duration; 3] = [
    std::time::Duration::from_secs(2),
    std::time::Duration::from_secs(10),
    std::time::Duration::from_secs(30),
];
#[cfg(test)]
const RETRY_DELAYS: [std::time::Duration; 3] = [std::time::Duration::from_millis(1); 3];

/// The reason a run stopped on an engine that stayed unavailable through
/// every retry; it is resumed on its own.
const UNAVAILABLE_REASON: &str =
    "the memory service is unavailable; the import resumes on its own within a few minutes";

/// What storing one batch of legacy items did.
#[derive(Debug, Default)]
struct BatchOutcome {
    /// Items stored (or replayed).
    stored: u64,
    /// The checkpoint after the last item handled, stored or skipped.
    checkpoint: Option<Checkpoint>,
    /// Why the import must stop (credential rejected, credits exhausted,
    /// engine unreachable).
    fatal: Option<String>,
    /// Whether `fatal` is transient: the engine stayed unavailable through
    /// every retry, so the background job resumes the run.
    transient: bool,
    /// Whether `fatal` is the account's credits running out: the run is
    /// paused until automatic runs are allowed again ([`BillingCheck`]).
    credits: bool,
}

/// Runs `call` again after each of [`RETRY_DELAYS`] while it fails with a
/// transient error, and returns its last result.
async fn with_retries<T, F, Fut>(mut call: F) -> tinymemory_api::Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = tinymemory_api::Result<T>>,
{
    let mut result = call().await;
    for delay in RETRY_DELAYS {
        match &result {
            Err(error) if error.is_transient() => {
                tracing::debug!(
                    delay_ms = u64::try_from(delay.as_millis()).unwrap_or(u64::MAX),
                    "[memory:import] engine unavailable; retrying"
                );
                tokio::time::sleep(delay).await;
                result = call().await;
            }
            _ => break,
        }
    }
    result
}

/// Whether a store failure is about the item (skip it and go on) rather than
/// the engine or the account (stop, so a resume retries from here).
///
/// Only a refusal of the item itself is skippable: malformed or too large
/// (`InvalidRequest`), not storable here (`Unsupported`, `NotFound`), or a
/// replay key already bound to a different body (`Conflict`). Everything else
/// would fail the next item the same way: a rejected credential, an exhausted
/// credit balance (an `Engine` error), an unreachable or overloaded engine
/// (`Unavailable`). Skipping those would advance the checkpoint past items
/// that were never stored and finish `Done` with nothing imported.
fn skips_item(error: &tinymemory_api::Error) -> bool {
    use tinymemory_api::Error as E;
    matches!(
        error,
        E::InvalidRequest(_) | E::Unsupported(_) | E::NotFound(_) | E::Conflict(_)
    )
}

/// The message an import stopped by `error` reports.
fn fatal_message(error: tinymemory_api::Error) -> String {
    if error.is_transient() {
        return UNAVAILABLE_REASON.to_string();
    }
    if is_insufficient_credits(&error) {
        return "not enough credits to import your memory; top up, then resume the import to \
                continue where it stopped"
            .to_string();
    }
    MemoryError::from(error).to_string()
}

/// Stores `batch` in one bulk call. If the engine refuses the batch because
/// of an item (see [`skips_item`]), the items are stored one at a time instead
/// so only the bad ones are skipped. Any other failure stops the import
/// without advancing the checkpoint past what was stored.
async fn store_batch(bound: &BoundEngine, batch: Vec<ImportedItem>) -> BatchOutcome {
    let Some(last) = batch.last() else {
        return BatchOutcome::default();
    };
    let last_checkpoint = last.checkpoint.clone();
    let items: Vec<_> = batch.iter().map(|imported| imported.item.clone()).collect();
    match with_retries(|| bound.engine.store_many(items.clone())).await {
        Ok(receipts) => {
            tracing::debug!(
                engine = %bound.id,
                count = receipts.len(),
                "[memory:import] batch stored"
            );
            return BatchOutcome {
                stored: receipts.len() as u64,
                checkpoint: Some(last_checkpoint),
                ..BatchOutcome::default()
            };
        }
        Err(error) if !skips_item(&error) => {
            return BatchOutcome {
                transient: error.is_transient(),
                credits: is_insufficient_credits(&error),
                fatal: Some(fatal_message(error)),
                ..BatchOutcome::default()
            };
        }
        Err(error) => {
            tracing::debug!(
                code = MemoryError::from(error).code(),
                "[memory:import] batch refused; storing one by one"
            );
        }
    }
    let mut outcome = BatchOutcome::default();
    for imported in batch {
        match with_retries(|| bound.engine.store(imported.item.clone())).await {
            Ok(_) => outcome.stored += 1,
            Err(error) if !skips_item(&error) => {
                outcome.transient = error.is_transient();
                outcome.credits = is_insufficient_credits(&error);
                outcome.fatal = Some(fatal_message(error));
                return outcome;
            }
            Err(error) => {
                tracing::debug!(
                    code = MemoryError::from(error).code(),
                    "[memory:import] item skipped"
                );
            }
        }
        outcome.checkpoint = Some(imported.checkpoint);
    }
    outcome
}

/// Imports running now, per workspace.
static RUNNING: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

#[derive(Debug, Default, Serialize, Deserialize)]
struct ImportFile {
    #[serde(default)]
    state: ImportState,
    /// The run stopped because the account ran out of credits; the
    /// background job resumes it once automatic runs are allowed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    paused_for_credits: bool,
    #[serde(default)]
    checkpoint: Checkpoint,
}

fn file_path(workspace_dir: &Path) -> PathBuf {
    workspace_dir.join("memory").join("import_state.json")
}

fn read_file(workspace_dir: &Path) -> ImportFile {
    std::fs::read_to_string(file_path(workspace_dir))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Persists `file` atomically (staged, synced, renamed): a crash mid-write
/// leaves the previous state, never a torn file that reads back as a fresh
/// start and re-sends everything.
fn write_file(workspace_dir: &Path, file: &ImportFile) {
    let result = serde_json::to_vec_pretty(file)
        .map_err(|error| error.to_string())
        .and_then(|json| {
            crate::security::keyring::file_store::write_atomic(&file_path(workspace_dir), &json)
                .map_err(|error| error.to_string())
        });
    if let Err(error) = result {
        tracing::warn!(error = %error, "[memory:import] writing import state failed");
    }
}

/// Counts what a legacy store at `workspace_dir` holds, or `None` when there
/// is no v1 store there. Blocking (SQLite).
pub fn count_legacy(workspace_dir: &Path) -> Option<ImportCounts> {
    let workspace = match LegacyWorkspace::open(workspace_dir) {
        Ok(workspace) => workspace,
        Err(error) => {
            tracing::debug!(error = %error, "[memory:import] no legacy store");
            return None;
        }
    };
    let mut counts = ImportCounts::default();
    for imported in workspace.items() {
        match imported {
            Ok(ImportedItem { item, .. }) => match item.kind() {
                ItemKind::Document => counts.documents += 1,
                ItemKind::Conversation => counts.conversations += 1,
                ItemKind::Learning => counts.learnings += 1,
            },
            Err(error) => {
                tracing::warn!(error = %error, "[memory:import] legacy store unreadable mid-scan");
                break;
            }
        }
    }
    Some(counts)
}

/// `memory_import_scan`.
pub async fn scan(config: &Config) -> MemoryResult<ImportScanView> {
    let workspace_dir = config.workspace_dir.clone();
    let counts = tokio::task::spawn_blocking(move || count_legacy(&workspace_dir))
        .await
        .map_err(|error| MemoryError::Engine(format!("import scan failed: {error}")))?;
    Ok(ImportScanView {
        found: counts.is_some(),
        counts,
    })
}

/// `memory_import_status`.
#[must_use]
pub fn status(config: &Config) -> ImportState {
    let mut state = read_file(&config.workspace_dir).state;
    let running = RUNNING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains(&config.workspace_dir);
    if state.phase == ImportPhase::Running && !running {
        state.phase = ImportPhase::Error;
        state.error = Some(state.error.unwrap_or_else(|| {
            "the import was interrupted; it resumes on its own within a few minutes, or resume \
             it now"
                .to_string()
        }));
    }
    state
}

/// Whether background work is paused right now. An automatic resume asks
/// before it starts and again before every batch it stores.
pub(crate) type PauseCheck = Arc<dyn Fn() -> bool + Send + Sync>;

/// Whether memory work that uploads on its own, with no user action (an
/// automatic import, or resuming one paused for credits), may run now.
pub(crate) type BillingCheck = Arc<dyn Fn(&Config) -> bool + Send + Sync>;

/// The one gate on automatic migration runs. A self-hosted CortexDB bills
/// nobody, so it always may. On the TinyHuman memory service an automatic
/// run spends the user's credits, so it may only while the backend's memory
/// free period is active.
///
/// The shared free-period check (`free_period_active()`) replaces the
/// TinyHuman half once it lands; until then the free period is unknown, and
/// unknown is not free.
pub(crate) fn automatic_run_allowed(config: &Config) -> bool {
    config.memory.engine.trim() == super::engine::CORTEXDB_ENGINE
}

/// The scheduler's pause, which includes being signed out.
fn scheduler_paused() -> bool {
    matches!(
        crate::cron::scheduler_gate::current_policy(),
        crate::cron::scheduler_gate::Policy::Paused { .. }
    )
}

/// Resumes an import the app quit in the middle of, if there is one, under
/// the scheduler's pause ([`resume_interrupted_with`]). Called from memory's
/// background job.
pub async fn resume_interrupted(config: &Config) -> bool {
    resume_interrupted_with(
        config,
        Arc::new(scheduler_paused),
        Arc::new(automatic_run_allowed),
    )
    .await
}

/// Resumes an interrupted import unless `paused` says background work is
/// paused.
///
/// Only a `Running` state with no live run counts: the user consented when
/// it started, and quitting the app is not a decision to stop. Nothing
/// resumes while background work is paused (which includes being signed
/// out), and a resumed run asks `paused` again before every batch, so a pause
/// that lands after the check stops it at the next batch. Either way the state
/// stays `Running` with its checkpoint, so a later tick resumes it. An import
/// that stopped on an error (credits exhausted, engine unreachable) stays
/// stopped until the user starts it again, and so does one whose automatic
/// resume could not start: that failure is persisted as `Error`, so a
/// failure that would recur does not loop. Returns whether a run was
/// started.
pub(crate) async fn resume_interrupted_with(
    config: &Config,
    paused: PauseCheck,
    billing: BillingCheck,
) -> bool {
    let file = read_file(&config.workspace_dir);
    let resumable = match file.state.phase {
        ImportPhase::Running => true,
        // Out of credits: only once automatic runs are allowed again.
        ImportPhase::Error => file.paused_for_credits && billing(config),
        _ => false,
    };
    if !resumable {
        return false;
    }
    let live = RUNNING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains(&config.workspace_dir);
    if live {
        return false;
    }
    if paused() {
        tracing::debug!("[memory:import] background paused; interrupted import left for later");
        return false;
    }
    match start_with(config, true, Some(paused)).await {
        Ok(state) => {
            tracing::info!(
                imported = state.imported,
                total = state.total,
                "[memory:import] resuming an interrupted import"
            );
            true
        }
        Err(error) => {
            tracing::warn!(
                code = error.code(),
                "[memory:import] interrupted import could not resume; stopped"
            );
            // Re-read so only the phase and reason change: the progress and
            // the checkpoint stay exactly as persisted, and the user's Resume
            // continues from there.
            let mut file = read_file(&config.workspace_dir);
            file.state.phase = ImportPhase::Error;
            file.state.error = Some(format!("the import could not resume: {error}"));
            write_file(&config.workspace_dir, &file);
            false
        }
    }
}

/// `memory_import_start`: requires `consent`, memory on, and a legacy store.
pub async fn start(config: &Config, consent: bool) -> MemoryResult<ImportState> {
    start_with(config, consent, None).await
}

/// [`start`], with the pause an automatic resume honors (`None` for an import
/// the user started, which runs to the end).
async fn start_with(
    config: &Config,
    consent: bool,
    paused: Option<PauseCheck>,
) -> MemoryResult<ImportState> {
    if !consent {
        return Err(MemoryError::invalid(
            "importing uploads local memory to the selected engine; pass consent: true",
        ));
    }
    let bound = engine::resolve(config).engine()?;
    let workspace_dir = config.workspace_dir.clone();
    let claimed = RUNNING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(workspace_dir.clone());
    if !claimed {
        return Ok(status(config));
    }
    let mut file = read_file(&workspace_dir);
    if file.state.phase == ImportPhase::Done {
        file = ImportFile::default();
    }
    // A resumed import already knows its total: only check the store is
    // still there, instead of reading all of it again before the run reads
    // it once more.
    let resuming = !file.checkpoint.is_start() && file.state.total > 0;
    let scan_dir = workspace_dir.clone();
    let total = tokio::task::spawn_blocking(move || {
        if resuming {
            LegacyWorkspace::open(&scan_dir).ok().map(|_| None)
        } else {
            count_legacy(&scan_dir)
                .map(|counts| Some(counts.documents + counts.conversations + counts.learnings))
        }
    })
    .await
    .ok()
    .flatten();
    let Some(total) = total else {
        RUNNING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&workspace_dir);
        return Err(MemoryError::invalid("no v1 memory store to import"));
    };
    file.state.phase = ImportPhase::Running;
    file.paused_for_credits = false;
    if let Some(total) = total {
        file.state.total = total;
    }
    file.state.error = None;
    write_file(&workspace_dir, &file);
    let state = file.state.clone();
    tracing::info!(total = state.total, "[memory:import] import started");
    tokio::spawn(async move {
        run(&workspace_dir, &bound, file, paused).await;
        RUNNING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&workspace_dir);
    });
    Ok(state)
}

async fn run(
    workspace_dir: &Path,
    bound: &BoundEngine,
    mut file: ImportFile,
    paused: Option<PauseCheck>,
) {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Result<ImportedItem, String>>(16);
    let reader_dir = workspace_dir.to_path_buf();
    let checkpoint = file.checkpoint.clone();
    let reader = tokio::task::spawn_blocking(move || {
        let workspace = match LegacyWorkspace::open(&reader_dir) {
            Ok(workspace) => workspace,
            Err(error) => {
                let _ = tx.blocking_send(Err(error.to_string()));
                return;
            }
        };
        for imported in workspace.items_from(&checkpoint) {
            let message = imported.map_err(|error| error.to_string());
            let stop = message.is_err();
            if tx.blocking_send(message).is_err() || stop {
                return;
            }
        }
    });
    let mut since_checkpoint = 0u64;
    let mut failure = None;
    let mut transient = false;
    let mut credits = false;
    let mut pausing = false;
    let mut batch: Vec<ImportedItem> = Vec::with_capacity(STORE_BATCH);
    let mut reading = true;
    while reading || !batch.is_empty() {
        if reading && batch.len() < STORE_BATCH {
            match rx.recv().await {
                Some(Ok(imported)) => {
                    batch.push(imported);
                    continue;
                }
                Some(Err(error)) => {
                    failure = Some(format!("reading the legacy store failed: {error}"));
                    reading = false;
                    batch.clear();
                    continue;
                }
                None => reading = false,
            }
        }
        if paused.as_ref().is_some_and(|paused| paused()) {
            pausing = true;
            break;
        }
        let outcome = store_batch(bound, std::mem::take(&mut batch)).await;
        file.state.imported += outcome.stored;
        if let Some(checkpoint) = outcome.checkpoint {
            file.checkpoint = checkpoint;
        }
        since_checkpoint += outcome.stored;
        if since_checkpoint >= CHECKPOINT_EVERY {
            write_file(workspace_dir, &file);
            since_checkpoint = 0;
        }
        if let Some(error) = outcome.fatal {
            failure = Some(error);
            transient = outcome.transient;
            credits = outcome.credits;
            break;
        }
    }
    drop(rx);
    let _ = reader.await;
    if pausing {
        // Left `Running` with its checkpoint: the next unpaused tick resumes it.
        tracing::info!(
            imported = file.state.imported,
            "[memory:import] background paused; import left to resume"
        );
        write_file(workspace_dir, &file);
        return;
    }
    match failure {
        // Left `Running` with its checkpoint and the reason: the next
        // background tick resumes it, as it does an import the app quit.
        Some(error) if transient => {
            tracing::info!(
                imported = file.state.imported,
                "[memory:import] engine unavailable; import left to resume"
            );
            file.state.error = Some(error);
        }
        Some(error) => {
            file.paused_for_credits = credits;
            tracing::warn!(
                imported = file.state.imported,
                "[memory:import] import stopped"
            );
            file.state.phase = ImportPhase::Error;
            file.state.error = Some(error);
        }
        None => {
            tracing::info!(
                imported = file.state.imported,
                "[memory:import] import finished"
            );
            file.state.phase = ImportPhase::Done;
            file.state.error = None;
        }
    }
    write_file(workspace_dir, &file);
}

#[cfg(test)]
#[path = "import_tests.rs"]
mod tests;
