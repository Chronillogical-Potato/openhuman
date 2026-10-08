//! Which channel each logged thread arrived on, so a channel's memory can be
//! forgotten when it is disconnected.
//!
//! TinyMemory's turn items carry the thread, not the channel. The session
//! host records the pairing when it logs a turn ([`record`]), in
//! `<workspace>/memory/channel_threads.json`; [`forget_channel`] forgets the
//! conversations of every thread a channel owns.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use tinymemory_api::{ForgetTarget, ItemKind, MetaFilter};

use crate::config::Config;

use super::engine;
use super::error::{MemoryError, MemoryResult};

/// Serialises read-modify-writes of the file in this process.
static LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn path(workspace_dir: &Path) -> PathBuf {
    workspace_dir.join("memory").join("channel_threads.json")
}

fn read(workspace_dir: &Path) -> BTreeMap<String, BTreeSet<String>> {
    std::fs::read_to_string(path(workspace_dir))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write(workspace_dir: &Path, all: &BTreeMap<String, BTreeSet<String>>) {
    let file = path(workspace_dir);
    let result = serde_json::to_vec_pretty(all)
        .map_err(std::io::Error::other)
        .and_then(|json| crate::memory::files::write_private(&file, &json));
    if let Err(error) = result {
        tracing::warn!(%error, "[memory:channels] writing channel threads failed");
    }
}

fn key(channel: &str) -> String {
    channel.trim().to_ascii_lowercase()
}

/// Records that `thread_id` arrived on `channel`.
pub fn record(workspace_dir: &Path, channel: &str, thread_id: &str) {
    let channel = key(channel);
    if channel.is_empty() || thread_id.trim().is_empty() {
        return;
    }
    let _guard = LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut all = read(workspace_dir);
    if all
        .entry(channel)
        .or_default()
        .insert(thread_id.to_string())
    {
        write(workspace_dir, &all);
    }
}

/// The threads `channel` owns.
#[must_use]
pub fn threads_of(workspace_dir: &Path, channel: &str) -> Vec<String> {
    read(workspace_dir)
        .get(&key(channel))
        .map(|threads| threads.iter().cloned().collect())
        .unwrap_or_default()
}

/// Forgets every conversation logged from `channel` and drops its record.
/// Memory off forgets nothing and reports zero.
pub async fn forget_channel(config: &Config, channel: &str) -> MemoryResult<usize> {
    let bound = match engine::resolve(config).engine() {
        Ok(bound) => bound,
        Err(MemoryError::Off(_)) => return Ok(0),
        Err(error) => return Err(error),
    };
    let mut forgotten = 0;
    for thread_id in threads_of(&config.workspace_dir, channel) {
        let filter = MetaFilter {
            thread_id: Some(thread_id),
            ..MetaFilter::kinds([ItemKind::Conversation])
        };
        forgotten += bound
            .engine
            .forget(ForgetTarget::Filter(filter))
            .await?
            .forgotten;
    }
    {
        let _guard = LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut all = read(&config.workspace_dir);
        if all.remove(&key(channel)).is_some() {
            write(&config.workspace_dir, &all);
        }
    }
    tracing::debug!(forgotten, "[memory:channels] channel forgotten");
    Ok(forgotten)
}

#[cfg(test)]
#[path = "channels_tests.rs"]
mod tests;
