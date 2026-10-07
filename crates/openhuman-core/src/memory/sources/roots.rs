//! Which layout roots each Composio connection's items were filed under.
//!
//! A connection's items go to its toolkit's brain source in the layout of
//! the source configured for it, and that layout can change between syncs
//! (a source's own `namespace` set, changed or removed). Forgetting a
//! connection reads `source:<toolkit>` under every root recorded here, so
//! items filed under an earlier root are forgotten with the rest. It lives
//! in `<workspace>/memory/connection_roots.json`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

/// Serialises every read and read-modify-write of the file in this process.
static LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

type Roots = BTreeMap<String, BTreeSet<String>>;

fn path(workspace_dir: &Path) -> PathBuf {
    workspace_dir.join("memory").join("connection_roots.json")
}

/// The file's contents. A file that exists but cannot be read is set aside
/// (`.corrupt`) with a warning rather than overwritten.
fn load(workspace_dir: &Path) -> Roots {
    let file = path(workspace_dir);
    let text = match std::fs::read_to_string(&file) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Roots::new(),
        Err(error) => {
            tracing::warn!(error = %error, "[memory:sources] connection roots unreadable");
            return set_aside(&file);
        }
    };
    serde_json::from_str(&text).unwrap_or_else(|error| {
        tracing::warn!(error = %error, "[memory:sources] connection roots unparsable");
        set_aside(&file)
    })
}

fn set_aside(file: &Path) -> Roots {
    if let Err(error) = std::fs::rename(file, file.with_extension("json.corrupt")) {
        tracing::warn!(error = %error, "[memory:sources] could not set the roots file aside");
    }
    Roots::new()
}

/// Writes through a temporary file and a rename, so a stop mid-write never
/// leaves a partial file.
fn save(workspace_dir: &Path, all: &Roots) -> std::io::Result<()> {
    let file = path(workspace_dir);
    let temp = file.with_extension("json.tmp");
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_vec_pretty(all).map_err(std::io::Error::other)?;
    std::fs::write(&temp, json)?;
    std::fs::rename(&temp, &file)
}

fn locked() -> std::sync::MutexGuard<'static, ()> {
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Records that `connection_id` files items under `root`.
///
/// # Errors
///
/// The record could not be written: the caller must not file items under
/// a root a later forget would not know.
pub fn record(workspace_dir: &Path, connection_id: &str, root: &str) -> std::io::Result<()> {
    let _guard = locked();
    let mut all = load(workspace_dir);
    if all
        .entry(connection_id.to_string())
        .or_default()
        .insert(root.to_string())
    {
        save(workspace_dir, &all)?;
    }
    Ok(())
}

/// Every root `connection_id` filed items under.
#[must_use]
pub fn of(workspace_dir: &Path, connection_id: &str) -> BTreeSet<String> {
    let _guard = locked();
    load(workspace_dir)
        .remove(connection_id)
        .unwrap_or_default()
}

/// Forgets `roots` of `connection_id`, once its items under them are gone.
/// A root recorded meanwhile (a sync during the forget) is kept.
pub fn forget(workspace_dir: &Path, connection_id: &str, roots: &BTreeSet<String>) {
    let _guard = locked();
    let mut all = load(workspace_dir);
    let Some(held) = all.get_mut(connection_id) else {
        return;
    };
    held.retain(|root| !roots.contains(root));
    if held.is_empty() {
        all.remove(connection_id);
    }
    if let Err(error) = save(workspace_dir, &all) {
        tracing::warn!(error = %error, "[memory:sources] writing connection roots failed");
    }
}

#[cfg(test)]
#[path = "roots_tests.rs"]
mod tests;
