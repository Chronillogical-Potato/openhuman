//! The migration's state on disk, `<workspace>/memory/layout_migration.json`.
//!
//! Written whole after every page, through a temporary file and a rename,
//! so a crash leaves the previous state or the new one, never a torn file.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::memory::error::{MemoryError, MemoryResult};

/// Where the migration is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Not started.
    #[default]
    Idle,
    /// Copying the legacy tree, page by page.
    Copying,
    /// Stopped where it was: paused by the scheduler, out of credits, or the
    /// engine unreachable. Resumes from the cursor.
    Paused,
    /// Every item it could copy is copied and verified.
    Copied,
    /// Removing the legacy copies of what was moved.
    Cleaning,
    /// The legacy tree holds nothing that was moved; what is left there (if
    /// anything) is in `failures` or `incomplete`.
    Cleaned,
}

/// An item that could not be copied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failure {
    /// Its id in the legacy tree.
    pub id: String,
    /// Why (an error code, never content).
    pub reason: String,
}

/// The migration's progress.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MigrationState {
    /// Where it is.
    pub phase: Phase,
    /// The legacy export cursor of the next page; `None` before the first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// Items copied and confirmed readable in the per-user tree.
    pub copied: u64,
    /// Of those, items the per-user tree already held (a resumed page).
    pub replayed: u64,
    /// Items that could not be copied.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<Failure>,
    /// Items the legacy tree holds only part of (a chunked document missing
    /// a piece).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub incomplete: Vec<String>,
    /// Why it is paused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Cleanup has started, so a pause resumes it, not the copy.
    #[serde(default)]
    pub cleaning: bool,
    /// Reads and writes moved to the per-user tree.
    #[serde(default)]
    pub switched: bool,
    /// The catch-up copy after the switch is done.
    #[serde(default)]
    pub caught_up: bool,
    /// The user agreed to take a legacy tree other accounts may share.
    #[serde(default)]
    pub takeover: bool,
}

/// The state file of `workspace_dir`.
#[must_use]
pub fn path(workspace_dir: &Path) -> PathBuf {
    workspace_dir.join("memory").join("layout_migration.json")
}

/// The saved state, or a fresh one when none was saved.
///
/// # Errors
///
/// A state file that cannot be read or parsed: the migration must not start
/// over and copy everything again on a damaged file.
pub fn load(workspace_dir: &Path) -> MemoryResult<MigrationState> {
    let file = path(workspace_dir);
    match std::fs::read(&file) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| {
            MemoryError::Engine(format!("layout migration state unreadable: {error}"))
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(MigrationState::default()),
        Err(error) => Err(MemoryError::Engine(format!(
            "layout migration state unreadable: {error}"
        ))),
    }
}

/// Saves `state` whole: a temporary file, synced, then renamed over the old.
///
/// # Errors
///
/// When the file cannot be written.
pub fn save(workspace_dir: &Path, state: &MigrationState) -> MemoryResult<()> {
    let file = path(workspace_dir);
    let write = || -> std::io::Result<()> {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let temp = file.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(state).map_err(std::io::Error::other)?;
        let mut out = std::fs::File::create(&temp)?;
        std::io::Write::write_all(&mut out, &bytes)?;
        out.sync_all()?;
        std::fs::rename(&temp, &file)
    };
    write()
        .map_err(|error| MemoryError::Engine(format!("layout migration state not saved: {error}")))
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
