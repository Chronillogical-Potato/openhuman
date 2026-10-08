//! A migration end to end: detect, copy, switch, catch up, clean up.
//!
//! - **Detect.** No legacy memory: switch at once, and that is all; there is
//!   no job, no banner and no prompt.
//! - **Gates.** A legacy tree other accounts may share (a self-hosted key)
//!   moves only with the user's consent ([`Trigger::Manual`] with
//!   `takeover`). An automatic run starts, and goes on page by page, only
//!   while moving costs the user nothing; when that ends mid-run it pauses
//!   where it is, and resumes in a later free window or when the user starts
//!   it.
//! - **Copy, then switch.** Reads and writes move to the per-user tree only
//!   after every item that could be copied is verified there, so no turn
//!   ever reads an empty memory.
//! - **Catch up.** Writes to the legacy tree between the copy and the switch
//!   are picked up by copying once more; everything copied before replays.
//! - **Clean up.** Last, because it drops held recall packs.
//!
//! Every step saves its progress, so [`run`] called again (after a pause, a
//! crash or a restart) continues where the last one stopped.

use async_trait::async_trait;

use super::claim::{self, ClaimKey};
use super::cleanup::cleanup;
use super::copy::{copy, legacy_present, Engines};
use super::map::Placement;
use super::state::{self, MigrationState, Phase};
use crate::config::Config;
use crate::memory::error::MemoryResult;

/// What started a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// The background job: runs only while moving is free.
    Auto,
    /// The user's "Migrate now"; `takeover` is their consent to take a
    /// legacy tree other accounts may share.
    Manual {
        /// Consent to take a shared legacy tree.
        takeover: bool,
    },
}

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// There was no legacy memory; the per-user tree is in use.
    NothingToMove,
    /// Not started: the legacy tree may be shared, and the user has not
    /// agreed to take it.
    NeedsTakeover,
    /// Not started: another account on this machine took the shared legacy
    /// tree.
    ClaimedElsewhere,
    /// Not started or not continued automatically: moving is not free now.
    NotFree,
    /// Stopped where it was; [`run`] again continues.
    Paused,
    /// Moved and cleaned up.
    Done,
}

/// What the migration needs from the host, behind one trait so the job is
/// tested without a TinyHumans session or a CortexDB server.
///
/// The real host (the engine binding and the layout setting) implements it
/// over `memory::engine` and `memory::scope`; tests implement it over two
/// in-memory engines.
#[async_trait]
pub trait LayoutHost: Send + Sync {
    /// The legacy-layout engine and the per-user engine for `config`'s
    /// account, on one endpoint and credential.
    ///
    /// # Errors
    ///
    /// When memory is off or the account has no per-user root.
    fn engines(&self, config: &Config) -> MemoryResult<Engines>;

    /// Where items go in the per-user tree.
    ///
    /// # Errors
    ///
    /// When the layout cannot be built.
    fn placement(&self, config: &Config) -> MemoryResult<Placement>;

    /// Whether reads and writes already use the per-user tree.
    fn is_switched(&self, config: &Config) -> bool;

    /// Moves reads and writes to the per-user tree: persists the layout
    /// setting and drops the cached engines. The real host loads the migrated
    /// person's own config file fresh, so a setting changed during a long
    /// migration is never reverted and an account switch mid-move never
    /// writes another person's config.
    ///
    /// # Errors
    ///
    /// When the setting cannot be saved.
    async fn switch(&self, config: &Config) -> MemoryResult<()>;

    /// Whether moving memory costs the user nothing right now (always, off
    /// the hosted engine).
    async fn free_now(&self, config: &Config) -> bool;

    /// The claim on the legacy tree when other accounts on this machine may
    /// share it (a self-hosted engine), so taking it needs the user's
    /// consent and only one account may; `None` when it is the account's own.
    ///
    /// # Errors
    ///
    /// When memory is off or the account has no per-user root.
    fn legacy_claim(&self, config: &Config) -> MemoryResult<Option<ClaimKey>>;
}

/// The most extra catch-up passes after an import that ended with its last
/// batch not confirmed listed.
const MAX_RECHECKS: u32 = 3;

/// Runs (or resumes) the migration of `config`'s account. `paused` is the
/// scheduler's own pause (background work held), asked before every page.
///
/// # Errors
///
/// When the state cannot be read or saved, the engines cannot be bound, or
/// the legacy tree cannot be read for a reason that is not account-wide.
pub async fn run<P>(
    config: &Config,
    host: &dyn LayoutHost,
    trigger: Trigger,
    paused: impl Fn() -> P,
) -> MemoryResult<Outcome>
where
    P: std::future::Future<Output = bool>,
{
    let dir = config.workspace_dir.as_path();
    let mut state = state::load(dir)?;
    // Cleaned with the import still unconfirmed (a stop between cleanup and
    // the re-check below) goes round once more rather than counting as done.
    if state.phase == Phase::Cleaned && !crate::memory::import::listed_unconfirmed(dir) {
        return Ok(Outcome::Done);
    }
    let engines = host.engines(config)?;
    if state.phase == Phase::Idle && !state.switched && !legacy_present(&*engines.legacy).await? {
        return nothing_to_move(config, host, &mut state).await;
    }
    if let Trigger::Manual { takeover: true } = trigger {
        state.takeover = true;
        state::save(dir, &state)?;
    }
    let shared = host.legacy_claim(config)?;
    if let Some(key) = &shared {
        if claim::held_by_other(key)? {
            return claimed_elsewhere(config, host, &mut state).await;
        }
        if !state.takeover {
            return Ok(Outcome::NeedsTakeover);
        }
        if !claim::take(key)? {
            return claimed_elsewhere(config, host, &mut state).await;
        }
    }
    let auto = trigger == Trigger::Auto;
    if auto && !host.free_now(config).await {
        return Ok(Outcome::NotFree);
    }
    let paused = &paused;
    let stop = move || async move { paused().await || (auto && !host.free_now(config).await) };
    let placement = host.placement(config)?;

    if !state.switched && !state.cleaning {
        copy(dir, &engines, &placement, &mut state, stop).await?;
        if state.phase != Phase::Copied {
            return Ok(Outcome::Paused);
        }
        if !host.is_switched(config) {
            host.switch(config).await?;
        }
        state.switched = true;
        state.cursor = None;
        state::save(dir, &state)?;
        tracing::info!("[memory:layout_migration] switched to the per-user tree");
    }
    if !state.caught_up && !state.cleaning {
        copy(dir, &engines, &placement, &mut state, stop).await?;
        if state.phase != Phase::Copied {
            return Ok(Outcome::Paused);
        }
        state.caught_up = true;
        state::save(dir, &state)?;
    }
    cleanup(
        dir,
        &engines,
        &placement,
        shared.is_some(),
        &mut state,
        stop,
    )
    .await?;
    // An import that ended with its last batch not confirmed listed may hold
    // items no copy saw yet: copy again on a later tick (at least once, then
    // while a pass still moves something new, at most MAX_RECHECKS times)
    // before finishing. A late item is never erased meanwhile (`cleanup`).
    if state.phase == Phase::Cleaned && crate::memory::import::listed_unconfirmed(dir) {
        let new = state.copied > state.replayed;
        if (state.rechecks == 0 || new) && state.rechecks < MAX_RECHECKS {
            state.rechecks += 1;
            state.caught_up = false;
            state.cleaning = false;
            state.cursor = None;
            state.phase = Phase::Copied;
            state::save(dir, &state)?;
            tracing::info!(
                rechecks = state.rechecks,
                "[memory:layout_migration] import not confirmed listed; copying again"
            );
            return Ok(Outcome::Paused);
        }
        crate::memory::import::clear_listed_unconfirmed(dir);
    }
    Ok(if state.phase == Phase::Cleaned {
        Outcome::Done
    } else {
        Outcome::Paused
    })
}

/// Another account took the shared legacy tree: this one has nothing to
/// move, and goes on in its own per-user tree like an account that never had
/// legacy memory, rather than in the tree the other account emptied.
async fn claimed_elsewhere(
    config: &Config,
    host: &dyn LayoutHost,
    state: &mut MigrationState,
) -> MemoryResult<Outcome> {
    nothing_to_move(config, host, state).await?;
    Ok(Outcome::ClaimedElsewhere)
}

async fn nothing_to_move(
    config: &Config,
    host: &dyn LayoutHost,
    state: &mut MigrationState,
) -> MemoryResult<Outcome> {
    if !host.is_switched(config) {
        host.switch(config).await?;
    }
    state.switched = true;
    state.caught_up = true;
    state.phase = Phase::Cleaned;
    state::save(&config.workspace_dir, state)?;
    tracing::info!("[memory:layout_migration] no legacy memory; per-user tree in use");
    Ok(Outcome::NothingToMove)
}

#[cfg(test)]
#[path = "job_tests.rs"]
mod tests;
