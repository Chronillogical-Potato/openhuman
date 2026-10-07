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

use super::cleanup::cleanup;
use super::copy::{copy, legacy_present};
use super::host::LayoutHost;
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
    /// Not started or not continued automatically: moving is not free now.
    NotFree,
    /// Stopped where it was; [`run`] again continues.
    Paused,
    /// Moved and cleaned up.
    Done,
}

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
    if state.phase == Phase::Cleaned {
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
    if host.shared_legacy(config) && !state.takeover {
        return Ok(Outcome::NeedsTakeover);
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
    cleanup(dir, &engines, &placement, &mut state, stop).await?;
    Ok(if state.phase == Phase::Cleaned {
        Outcome::Done
    } else {
        Outcome::Paused
    })
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
