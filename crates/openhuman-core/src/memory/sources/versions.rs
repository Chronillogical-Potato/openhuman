//! Which stored item holds the current version of each connector record.
//!
//! A memory item's id is a digest of its content, so a record edited
//! upstream (a Notion page, a Linear issue) syncs as a new item and the old
//! version stays beside it. This file remembers, per record
//! ([`key`]), the id of the item last stored for it. A sync pass
//! ([`begin`]) records the ids it stored and the records that came back
//! empty, and hands back the ids no record uses any more; the caller
//! forgets them and then [`settle`]s them. Until settled they stay pending
//! and are handed back again by the next pass, so a failed forget is
//! retried and never orphans a version. It lives in
//! `<workspace>/memory/connector_items.json`.
//!
//! A workspace without the file (a new device) cannot tell an old version
//! from a new one, so versions stored before it existed stay until the
//! source is re-synced from scratch.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use serde::{Deserialize, Serialize};

/// Serialises every read and read-modify-write of the file in this process.
static LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// The file's contents.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Versions {
    /// Record key → the id of the item holding its current version.
    #[serde(default)]
    items: BTreeMap<String, String>,
    /// Ids no record uses any more, not yet forgotten.
    #[serde(default)]
    stale: BTreeSet<String>,
}

fn path(workspace_dir: &Path) -> PathBuf {
    workspace_dir.join("memory").join("connector_items.json")
}

// ponytail: the whole map is read and rewritten per sync pass (~100 B a
// record, so ~1 MB at 10k records); a keyed store if accounts grow past that.
/// The file's contents, or `None` when it cannot be read right now: the
/// caller then changes nothing. A file that reads but does not parse is set
/// aside (`.corrupt`) with a warning rather than overwritten, and tracking
/// starts again from empty.
fn load(workspace_dir: &Path) -> Option<Versions> {
    let file = path(workspace_dir);
    let text = match std::fs::read_to_string(&file) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Some(Versions::default())
        }
        Err(error) => {
            tracing::warn!(error = %error, "[memory:sources] connector item versions unreadable; skipping");
            return None;
        }
    };
    match serde_json::from_str(&text) {
        Ok(versions) => Some(versions),
        Err(error) => {
            tracing::warn!(error = %error, "[memory:sources] connector item versions unparsable");
            set_aside(&file)
        }
    }
}

/// Moves an unparsable file to `.corrupt` and starts from empty, or, when
/// it cannot be moved, changes nothing (`None`), so a later save never
/// overwrites the only copy.
fn set_aside(file: &Path) -> Option<Versions> {
    match std::fs::rename(file, file.with_extension("json.corrupt")) {
        Ok(()) => Some(Versions::default()),
        Err(error) => {
            tracing::warn!(error = %error, "[memory:sources] could not set the versions file aside; skipping");
            None
        }
    }
}

/// Writes through a temporary file and a rename, so a stop mid-write never
/// leaves a partial file.
/// (`std::fs::rename` replaces an existing file on every platform.)
fn save(workspace_dir: &Path, versions: &Versions) -> bool {
    let file = path(workspace_dir);
    let temp = file.with_extension("json.tmp");
    let result = file
        .parent()
        .map_or(Ok(()), crate::memory::files::create_private_dir_all)
        .and_then(|()| {
            let json = serde_json::to_vec(versions).map_err(std::io::Error::other)?;
            crate::memory::files::write_private(&temp, &json)?;
            std::fs::rename(&temp, &file)
        });
    if let Err(error) = &result {
        tracing::warn!(error = %error, "[memory:sources] writing connector item versions failed");
    }
    result.is_ok()
}

fn locked() -> std::sync::MutexGuard<'static, ()> {
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The key one record of `connection_id` is remembered under. The
/// connection id is length-prefixed, so no pair of ids shares a key.
#[must_use]
pub fn key(connection_id: &str, item_id: &str) -> String {
    format!("{}{item_id}", prefix(connection_id))
}

fn prefix(connection_id: &str) -> String {
    format!("{}:{connection_id}:", connection_id.len())
}

/// Starts a sync pass: `stored` records (key, id now stored) and `emptied`
/// records (keys that came back with no content). Remembers their state and
/// returns every id to forget: those no record uses any more, and those an
/// earlier pass could not forget, even when this pass stored nothing.
/// Forget them, then [`settle`] them. Hands back nothing when the state
/// cannot be read or written, so no id is forgotten unrecorded.
#[must_use]
pub fn begin(workspace_dir: &Path, stored: &[(String, String)], emptied: &[String]) -> Vec<String> {
    let _guard = locked();
    let Some(mut versions) = load(workspace_dir) else {
        return Vec::new();
    };
    if stored.is_empty() && emptied.is_empty() {
        return versions.stale.into_iter().collect();
    }
    let mut replaced = Vec::new();
    for (key, id) in stored {
        if let Some(old) = versions.items.insert(key.clone(), id.clone()) {
            if old != *id {
                replaced.push(old);
            }
        }
    }
    for key in emptied {
        replaced.extend(versions.items.remove(key));
    }
    versions.stale.extend(replaced);
    // An id another record still holds (two records with the same content)
    // is not stale.
    let current: BTreeSet<&String> = versions.items.values().collect();
    versions.stale.retain(|id| !current.contains(id));
    if !save(workspace_dir, &versions) {
        return Vec::new();
    }
    versions.stale.into_iter().collect()
}

/// Marks `forgotten` as gone: they are no longer handed back.
pub fn settle(workspace_dir: &Path, forgotten: &[String]) {
    if forgotten.is_empty() {
        return;
    }
    let _guard = locked();
    let Some(mut versions) = load(workspace_dir) else {
        return;
    };
    for id in forgotten {
        versions.stale.remove(id);
    }
    let _ = save(workspace_dir, &versions);
}

/// Forgets every record of `connection_id`, once its items are gone.
pub fn drop_connection(workspace_dir: &Path, connection_id: &str) {
    let _guard = locked();
    let Some(mut versions) = load(workspace_dir) else {
        return;
    };
    let prefix = prefix(connection_id);
    let before = versions.items.len();
    versions.items.retain(|key, _| !key.starts_with(&prefix));
    if versions.items.len() != before {
        let _ = save(workspace_dir, &versions);
    }
}

#[cfg(test)]
#[path = "versions_tests.rs"]
mod tests;
