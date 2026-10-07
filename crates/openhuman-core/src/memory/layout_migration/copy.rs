//! The copy pass: the legacy tree into the per-user tree, page by page.
//!
//! Each page is exported whole from the legacy engine
//! ([`tinymemory_api::MemoryEngine::export`]), placed ([`Placement`]),
//! stored in the per-user engine, and read back by id before the state
//! moves past it. A page is never marked copied before its items are
//! readable where they now live, and a crash or a pause repeats at most one
//! page: storing an item the tree already holds is a replay.

use std::path::Path;
use std::sync::Arc;

use tinymemory_api::{GetRequest, ItemId, ListRequest, MemoryEngine, MetaFilter, StoreItem};

use super::map::Placement;
use super::state::{self, Failure, MigrationState, Phase};
use crate::memory::error::{MemoryError, MemoryResult};
use crate::memory::import::skips_item;

/// Items one page exports and stores.
pub const PAGE: usize = 50;

/// The two engines a migration works between, on one endpoint and
/// credential: the legacy tree it reads, and the per-user tree it writes.
pub struct Engines {
    /// The legacy layout (`app:tinymemory/…`).
    pub legacy: Arc<dyn MemoryEngine>,
    /// The per-user layout.
    pub tree: Arc<dyn MemoryEngine>,
}

/// Whether the legacy tree holds anything to move: one export of one item.
///
/// # Errors
///
/// The legacy engine's failures, including a tree too large to list whole.
pub async fn legacy_present(legacy: &dyn MemoryEngine) -> MemoryResult<bool> {
    let page = legacy
        .export(ListRequest::new(MetaFilter::default(), 1))
        .await?;
    Ok(!page.items.is_empty() || !page.incomplete.is_empty())
}

/// Copies from `state.cursor` to the end of the legacy tree, saving the
/// state after every page. Returns with [`Phase::Copied`] at the end, or
/// [`Phase::Paused`] when `paused` says so or the account cannot go on
/// (credits, credential, engine unreachable).
///
/// # Errors
///
/// When the state cannot be saved, or the legacy tree cannot be read for a
/// reason that is not account-wide.
pub async fn copy(
    workspace_dir: &Path,
    engines: &Engines,
    placement: &Placement,
    state: &mut MigrationState,
    paused: impl Fn() -> bool,
) -> MemoryResult<()> {
    state.phase = Phase::Copying;
    state.error = None;
    state::save(workspace_dir, state)?;
    loop {
        if paused() {
            return pause(
                workspace_dir,
                state,
                "background work is paused".to_string(),
            );
        }
        let mut request = ListRequest::new(MetaFilter::default(), PAGE);
        request.cursor = state.cursor.clone();
        let page = match engines.legacy.export(request).await {
            Ok(page) => page,
            Err(error) => {
                let error = MemoryError::from(error);
                if error.is_account_wide() {
                    return pause(workspace_dir, state, error.to_string());
                }
                return Err(error);
            }
        };
        state
            .incomplete
            .extend(page.incomplete.into_iter().map(|id| id.0));
        let mut placed = Vec::with_capacity(page.items.len());
        for exported in page.items {
            match placement.place(exported.item) {
                Ok(item) => placed.push((exported.id.0, item)),
                Err(error) => state.failures.push(Failure {
                    id: exported.id.0,
                    reason: error.code().to_string(),
                }),
            }
        }
        if let Err(error) = store_and_check(engines.tree.as_ref(), placed, state).await {
            return pause(workspace_dir, state, error.to_string());
        }
        state.cursor = page.next_cursor;
        if state.cursor.is_none() {
            state.phase = Phase::Copied;
            state::save(workspace_dir, state)?;
            tracing::info!(
                copied = state.copied,
                failures = state.failures.len(),
                incomplete = state.incomplete.len(),
                "[memory:layout_migration] legacy tree copied"
            );
            return Ok(());
        }
        state::save(workspace_dir, state)?;
    }
}

fn pause(workspace_dir: &Path, state: &mut MigrationState, why: String) -> MemoryResult<()> {
    tracing::info!(%why, copied = state.copied, "[memory:layout_migration] paused");
    state.phase = Phase::Paused;
    state.error = Some(why);
    state::save(workspace_dir, state)
}

/// Stores `placed` in the per-user tree and reads every item back by id.
/// An item the engine refuses on its own is recorded as a failure; an
/// account-wide failure stops the page (the caller pauses, and the page is
/// sent again on resume).
async fn store_and_check(
    tree: &dyn MemoryEngine,
    placed: Vec<(String, StoreItem)>,
    state: &mut MigrationState,
) -> Result<(), MemoryError> {
    if placed.is_empty() {
        return Ok(());
    }
    let items: Vec<StoreItem> = placed.iter().map(|(_, item)| item.clone()).collect();
    let mut stored: Vec<(String, ItemId, bool)> = Vec::new();
    match tree.store_many(items).await {
        Ok(receipts) => stored.extend(
            placed
                .iter()
                .zip(receipts)
                .map(|((legacy, _), receipt)| (legacy.clone(), receipt.id, receipt.replayed)),
        ),
        Err(error) if !skips_item(&error) => return Err(error.into()),
        Err(_) => {
            for (legacy, item) in placed {
                match tree.store(item).await {
                    Ok(receipt) => stored.push((legacy, receipt.id, receipt.replayed)),
                    Err(error) if !skips_item(&error) => return Err(error.into()),
                    Err(error) => state.failures.push(Failure {
                        id: legacy,
                        reason: MemoryError::from(error).code().to_string(),
                    }),
                }
            }
        }
    }
    let ids: Vec<ItemId> = stored.iter().map(|(_, id, _)| id.clone()).collect();
    let found = tree
        .get(GetRequest { ids, reach: None })
        .await
        .map_err(MemoryError::from)?;
    for (legacy, id, replayed) in stored {
        if found.iter().any(|hit| hit.id == id) {
            state.copied += 1;
            state.replayed += u64::from(replayed);
        } else {
            state.failures.push(Failure {
                id: legacy,
                reason: "not_readable_after_store".to_string(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "copy_tests.rs"]
mod tests;
