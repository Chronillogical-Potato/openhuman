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

/// Serialises read-modify-writes of the file in this process.
static LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn path(workspace_dir: &Path) -> PathBuf {
    workspace_dir.join("memory").join("connection_roots.json")
}

fn read(workspace_dir: &Path) -> BTreeMap<String, BTreeSet<String>> {
    std::fs::read_to_string(path(workspace_dir))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write(workspace_dir: &Path, all: &BTreeMap<String, BTreeSet<String>>) {
    let file = path(workspace_dir);
    let result = file
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| {
            let json = serde_json::to_vec_pretty(all).map_err(std::io::Error::other)?;
            std::fs::write(&file, json)
        });
    if let Err(error) = result {
        tracing::warn!(error = %error, "[memory:sources] writing connection roots failed");
    }
}

/// Records that `connection_id` filed items under `root`.
pub fn record(workspace_dir: &Path, connection_id: &str, root: &str) {
    let _guard = LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut all = read(workspace_dir);
    if all
        .entry(connection_id.to_string())
        .or_default()
        .insert(root.to_string())
    {
        write(workspace_dir, &all);
    }
}

/// Every root `connection_id` filed items under.
#[must_use]
pub fn of(workspace_dir: &Path, connection_id: &str) -> BTreeSet<String> {
    read(workspace_dir)
        .remove(connection_id)
        .unwrap_or_default()
}

/// Forgets `connection_id`'s roots, once its items are gone.
pub fn drop_connection(workspace_dir: &Path, connection_id: &str) {
    let _guard = LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut all = read(workspace_dir);
    if all.remove(connection_id).is_some() {
        write(workspace_dir, &all);
    }
}
