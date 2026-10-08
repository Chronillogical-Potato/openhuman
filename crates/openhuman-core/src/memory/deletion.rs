//! Deletions that must reach the engine, whenever it is reachable.
//!
//! Deleting a chat thread, forgetting a channel (one thread deletion per
//! thread it owned) or removing a source asks the engine to delete for good. When memory is off
//! (signed out, no credential) the engine cannot be reached, and when it
//! fails the deletion has not happened. Either way the deletion is recorded
//! here, in `<workspace>/memory/pending_deletions.json`, and [`drain`] runs
//! it again on the next sign-in ([`super::bus`], on `CredentialChanged`) and
//! on every background tick, until it succeeds.
//!
//! Every deletion is hard: a forget by filter removes the matched events by
//! `memory_ids` with an explicit `redact_events` cascade.

use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use serde::{Deserialize, Serialize};
use tinymemory_api::{ForgetTarget, ItemKind, MetaFilter};

use crate::config::Config;

use super::engine;
use super::error::{MemoryError, MemoryResult};

/// One deletion still owed to the engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PendingDeletion {
    /// A deleted chat thread's conversation memory.
    Thread {
        /// The thread.
        thread_id: String,
    },
    /// Every item a removed memory source stored.
    Source {
        /// The source id.
        source_id: String,
    },
}

impl PendingDeletion {
    /// A short, content-free label for logs.
    fn label(&self) -> &'static str {
        match self {
            Self::Thread { .. } => "thread",
            Self::Source { .. } => "source",
        }
    }
}

/// Serialises every read-modify-write of the file in this process.
static LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Serialises drains, so two triggers never run one deletion twice at once.
static DRAINING: LazyLock<tokio::sync::Mutex<()>> = LazyLock::new(|| tokio::sync::Mutex::new(()));

fn path(workspace_dir: &Path) -> PathBuf {
    workspace_dir.join("memory").join("pending_deletions.json")
}

fn locked() -> std::sync::MutexGuard<'static, ()> {
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn read(workspace_dir: &Path) -> Vec<PendingDeletion> {
    let file = path(workspace_dir);
    let Ok(text) = std::fs::read_to_string(&file) else {
        return Vec::new();
    };
    serde_json::from_str(&text).unwrap_or_else(|error| {
        tracing::warn!(%error, "[memory:deletion] pending deletions unparsable; set aside");
        if let Err(error) = std::fs::rename(&file, file.with_extension("json.corrupt")) {
            tracing::warn!(%error, "[memory:deletion] could not set the file aside");
        }
        Vec::new()
    })
}

/// Writes (owner-only, like the other memory files) through a temporary file and a rename, so a stop mid-write never
/// leaves a partial file and loses the queue.
fn write(workspace_dir: &Path, all: &[PendingDeletion]) {
    let file = path(workspace_dir);
    if all.is_empty() {
        match std::fs::remove_file(&file) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => tracing::warn!(%error, "[memory:deletion] clearing the queue failed"),
        }
        return;
    }
    let temp = file.with_extension("json.tmp");
    let result = file
        .parent()
        .map_or(Ok(()), super::files::create_private_dir_all)
        .and_then(|()| {
            let json = serde_json::to_vec_pretty(all).map_err(std::io::Error::other)?;
            super::files::write_private(&temp, &json)?;
            std::fs::rename(&temp, &file)
        });
    if let Err(error) = result {
        tracing::warn!(%error, "[memory:deletion] writing pending deletions failed");
    }
}

/// Records `deletion` to run on the next [`drain`]. A deletion already
/// queued is not queued twice.
pub fn enqueue(workspace_dir: &Path, deletion: PendingDeletion) {
    let _guard = locked();
    let mut all = read(workspace_dir);
    if all.contains(&deletion) {
        return;
    }
    tracing::info!(
        kind = deletion.label(),
        "[memory:deletion] deletion queued until memory is reachable"
    );
    all.push(deletion);
    write(workspace_dir, &all);
}

/// Every deletion still owed, oldest first.
#[must_use]
pub fn pending(workspace_dir: &Path) -> Vec<PendingDeletion> {
    let _guard = locked();
    read(workspace_dir)
}

fn settle(workspace_dir: &Path, deletion: &PendingDeletion) {
    let _guard = locked();
    let mut all = read(workspace_dir);
    let before = all.len();
    all.retain(|queued| queued != deletion);
    if all.len() != before {
        write(workspace_dir, &all);
    }
}

/// Runs `deletion` now. Memory off is [`MemoryError::Off`], which the
/// caller decides how to treat.
async fn run(config: &Config, deletion: &PendingDeletion) -> MemoryResult<usize> {
    match deletion {
        PendingDeletion::Thread { thread_id } => forget_thread_now(config, thread_id).await,
        PendingDeletion::Source { source_id } => {
            super::sources::forget_items(config, source_id).await
        }
    }
}

/// Runs every queued deletion, settling each one that succeeds. Stops at
/// once when memory is off (nothing can run), and keeps a deletion that
/// fails for the next drain. Returns how many deletions settled.
pub async fn drain(config: &Config) -> usize {
    let _draining = DRAINING.lock().await;
    let queued = pending(&config.workspace_dir);
    if queued.is_empty() {
        return 0;
    }
    if !engine::resolve(config).is_on() {
        tracing::debug!(
            queued = queued.len(),
            "[memory:deletion] memory off; pending deletions wait"
        );
        return 0;
    }
    let mut settled = 0;
    for deletion in queued {
        // Settled first: the deletion re-queues itself if it cannot finish.
        settle(&config.workspace_dir, &deletion);
        match run(config, &deletion).await {
            Ok(forgotten) => {
                settled += 1;
                tracing::info!(
                    kind = deletion.label(),
                    forgotten,
                    "[memory:deletion] pending deletion completed"
                );
            }
            Err(error) => {
                enqueue(&config.workspace_dir, deletion.clone());
                tracing::warn!(
                    kind = deletion.label(),
                    code = error.code(),
                    "[memory:deletion] pending deletion failed; kept for the next drain"
                );
            }
        }
    }
    settled
}

/// Forgets every conversation item of `thread_id` now.
async fn forget_thread_now(config: &Config, thread_id: &str) -> MemoryResult<usize> {
    let bound = engine::resolve(config).engine()?;
    let filter = MetaFilter {
        thread_id: Some(thread_id.to_string()),
        ..MetaFilter::kinds([ItemKind::Conversation])
    };
    let report = bound.engine.forget(ForgetTarget::Filter(filter)).await?;
    tracing::debug!(
        forgotten = report.forgotten,
        "[memory:deletion] thread conversation memory forgotten"
    );
    Ok(report.forgotten)
}

/// Forgets a deleted thread's conversation memory, for good. Memory off or
/// a failed forget queues the deletion for the next [`drain`] rather than
/// failing the caller: the thread itself is already gone. Returns how many
/// items went now.
pub async fn forget_thread(config: &Config, thread_id: &str) -> usize {
    if thread_id.trim().is_empty() {
        return 0;
    }
    let deletion = PendingDeletion::Thread {
        thread_id: thread_id.to_string(),
    };
    match forget_thread_now(config, thread_id).await {
        Ok(forgotten) => forgotten,
        Err(MemoryError::Off(_)) => {
            enqueue(&config.workspace_dir, deletion);
            0
        }
        Err(error) => {
            tracing::warn!(
                code = error.code(),
                "[memory:deletion] forgetting a deleted thread failed; queued"
            );
            enqueue(&config.workspace_dir, deletion);
            0
        }
    }
}

#[cfg(test)]
#[path = "deletion_tests.rs"]
mod tests;
