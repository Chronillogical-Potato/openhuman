//! Storing past chats: conversations that predate automatic ingestion.
//!
//! Live ingestion ([`super::record_turn`]) only sees turns committed while it
//! is on. This walks the thread store instead and stores every earlier turn
//! of every thread, in the same `batch_turns` batches and with the same
//! metadata live ingestion uses ([`Batch::into_item`]), plus a
//! [`BACKFILL_TAG`] so they can be told apart.
//!
//! - **Turns.** A thread's messages become turns the way the chat shows them:
//!   each user message opens a turn and the replies after it, up to the next
//!   user message, are its answer. Replies before any user message form a turn
//!   with no user side.
//! - **No overlap with live ingestion.** Live ingestion counts the turns it has
//!   taken per thread (the counter in `conversations_state.json`); those are
//!   the thread's most recent turns, so the backfill stops short of them.
//! - **Resumable and repeatable.** How far each thread has been stored is kept
//!   in `<workspace>/memory/conversations_backfill.json`, so an interrupted run
//!   resumes and a later run sends only what is new. A re-sent batch is a
//!   replay (the item id is a content digest), never a duplicate.
//! - **Consent.** It uploads chat history to the selected engine, so
//!   [`start`] refuses without `consent`.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::memory::engine::{self, BoundEngine};
use crate::memory::error::{MemoryError, MemoryResult};
use crate::memory::ops::store_many_on;
use crate::memory::types::ImportPhase;
use crate::threads::store::blocking as threads;
use crate::threads::store::ConversationMessage;

use super::buffer::{Batch, CommittedTurn};

/// The tag every backfilled conversation carries.
pub const BACKFILL_TAG: &str = "backfill";

/// Conversation items per bulk store (`MemoryEngine::store_many`).
const STORE_GROUP: usize = 25;

/// Backfills running now, per workspace.
static RUNNING: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// Progress of the last (or current) backfill.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackfillState {
    /// Idle, running, done, or stopped on an error.
    pub phase: ImportPhase,
    /// Threads with turns to store when the run started.
    pub threads_total: u64,
    /// Of those, threads finished.
    pub threads_done: u64,
    /// Turns stored by the run.
    pub turns_stored: u64,
    /// Conversation items stored by the run.
    pub items_stored: u64,
    /// Why it stopped, when it failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// When the last run finished.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
}

/// `memory_conversations_backfill_status` / `_start` result.
#[derive(Debug, Clone, Serialize)]
pub struct BackfillView {
    /// The run's progress.
    pub state: BackfillState,
    /// Threads that still have earlier turns to store.
    pub pending_threads: u64,
    /// Turns still to store across them.
    pub pending_turns: u64,
}

/// `memory_conversations_backfill_start` params.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct BackfillStartParams {
    /// The caller agrees to upload past chats to the engine.
    #[serde(default)]
    pub consent: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct BackfillFile {
    #[serde(default)]
    state: BackfillState,
    /// Turns stored so far, per thread.
    #[serde(default)]
    stored: BTreeMap<String, u32>,
}

fn file_path(workspace_dir: &Path) -> PathBuf {
    workspace_dir
        .join("memory")
        .join("conversations_backfill.json")
}

fn read_file(workspace_dir: &Path) -> BackfillFile {
    std::fs::read_to_string(file_path(workspace_dir))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write_file(workspace_dir: &Path, file: &BackfillFile) {
    let path = file_path(workspace_dir);
    let result = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| {
            let json = serde_json::to_vec_pretty(file).map_err(std::io::Error::other)?;
            std::fs::write(&path, json)
        });
    if let Err(error) = result {
        tracing::warn!(error = %error, "[memory:backfill] writing state failed");
    }
}

fn parse_time(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|at| at.with_timezone(&Utc))
}

/// A thread's messages as turns, oldest first.
#[must_use]
pub fn turns_of(thread_id: &str, messages: &[ConversationMessage]) -> Vec<CommittedTurn> {
    let mut turns: Vec<CommittedTurn> = Vec::new();
    for message in messages {
        let text = message.content.trim();
        if text.is_empty() {
            continue;
        }
        let at = parse_time(&message.created_at);
        if message.sender.eq_ignore_ascii_case("user") {
            turns.push(CommittedTurn {
                thread_id: thread_id.to_string(),
                agent_id: None,
                // Past chats are the main agent's: the shared root.
                namespace: tinymemory::Namespace::ROOT,
                workspace: None,
                channel: None,
                user: text.to_string(),
                assistant: String::new(),
                tool_calls: Vec::new(),
                at: at.unwrap_or_else(Utc::now),
            });
            continue;
        }
        let needs_turn = turns.is_empty();
        if needs_turn {
            turns.push(CommittedTurn {
                thread_id: thread_id.to_string(),
                agent_id: None,
                // Past chats are the main agent's: the shared root.
                namespace: tinymemory::Namespace::ROOT,
                workspace: None,
                channel: None,
                user: String::new(),
                assistant: String::new(),
                tool_calls: Vec::new(),
                at: at.unwrap_or_else(Utc::now),
            });
        }
        if let Some(turn) = turns.last_mut() {
            if !turn.assistant.is_empty() {
                turn.assistant.push_str("\n\n");
            }
            turn.assistant.push_str(text);
            if let Some(at) = at {
                turn.at = at;
            }
        }
    }
    turns
}

/// Which turns of a thread with `total` turns the backfill should store:
/// from what it already stored up to (not including) the ones live
/// ingestion took.
#[must_use]
pub fn pending_range(total: usize, live_taken: u32, already_stored: u32) -> std::ops::Range<usize> {
    let end = total.saturating_sub(live_taken as usize);
    let start = (already_stored as usize).min(end);
    start..end
}

/// The batches storing `turns[range]`, `batch_turns` per item, tagged.
#[must_use]
pub fn batches(
    thread_id: &str,
    turns: &[CommittedTurn],
    range: std::ops::Range<usize>,
    batch_turns: u32,
) -> Vec<Batch> {
    let size = batch_turns.max(1) as usize;
    let mut out = Vec::new();
    let mut first = range.start;
    while first < range.end {
        let end = (first + size).min(range.end);
        out.push(Batch {
            thread_id: thread_id.to_string(),
            first: u32::try_from(first).unwrap_or(u32::MAX),
            turns: turns[first..end].to_vec(),
        });
        first = end;
    }
    out
}

/// One thread's plan: its turns and the range still to store.
struct ThreadPlan {
    thread_id: String,
    turns: Vec<CommittedTurn>,
    range: std::ops::Range<usize>,
}

async fn plan(
    workspace_dir: &Path,
    stored: &BTreeMap<String, u32>,
) -> MemoryResult<Vec<ThreadPlan>> {
    let listed = threads::list_threads(workspace_dir.to_path_buf())
        .await
        .map_err(|error| MemoryError::Engine(format!("reading chat threads failed: {error}")))?;
    let mut plans = Vec::new();
    for thread in listed {
        let messages = match threads::get_messages(workspace_dir.to_path_buf(), thread.id.clone())
            .await
        {
            Ok(messages) => messages,
            Err(error) => {
                tracing::warn!(error = %error, "[memory:backfill] a thread was unreadable; skipped");
                continue;
            }
        };
        let turns = turns_of(&thread.id, &messages);
        let range = pending_range(
            turns.len(),
            super::live_turns_taken(workspace_dir, &thread.id),
            stored.get(&thread.id).copied().unwrap_or(0),
        );
        if !range.is_empty() {
            plans.push(ThreadPlan {
                thread_id: thread.id,
                turns,
                range,
            });
        }
    }
    Ok(plans)
}

fn is_running(workspace_dir: &Path) -> bool {
    RUNNING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains(workspace_dir)
}

/// `memory_conversations_backfill_status`.
pub async fn status(config: &Config) -> MemoryResult<BackfillView> {
    let workspace_dir = &config.workspace_dir;
    let file = read_file(workspace_dir);
    let mut state = file.state;
    if state.phase == ImportPhase::Running && !is_running(workspace_dir) {
        state.phase = ImportPhase::Error;
        state.error = Some("the sync was interrupted; start it again to resume".to_string());
    }
    let plans = plan(workspace_dir, &file.stored).await?;
    Ok(BackfillView {
        state,
        pending_threads: plans.len() as u64,
        pending_turns: plans.iter().map(|p| p.range.len() as u64).sum(),
    })
}

/// `memory_conversations_backfill_start`: requires `consent` and memory on.
pub async fn start(config: &Config, params: BackfillStartParams) -> MemoryResult<BackfillView> {
    if !params.consent {
        return Err(MemoryError::invalid(
            "storing past chats uploads them to the selected engine; pass consent: true",
        ));
    }
    let bound = engine::resolve(config).engine()?;
    let workspace_dir = config.workspace_dir.clone();
    let claimed = RUNNING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(workspace_dir.clone());
    if !claimed {
        return status(config).await;
    }
    let mut file = read_file(&workspace_dir);
    let plans = match plan(&workspace_dir, &file.stored).await {
        Ok(plans) => plans,
        Err(error) => {
            release(&workspace_dir);
            return Err(error);
        }
    };
    let pending_turns: u64 = plans.iter().map(|p| p.range.len() as u64).sum();
    file.state = BackfillState {
        phase: ImportPhase::Running,
        threads_total: plans.len() as u64,
        ..BackfillState::default()
    };
    write_file(&workspace_dir, &file);
    let view = BackfillView {
        state: file.state.clone(),
        pending_threads: plans.len() as u64,
        pending_turns,
    };
    tracing::info!(
        threads = plans.len(),
        turns = pending_turns,
        "[memory:backfill] started"
    );
    let batch_turns = config.memory.conversations.batch_turns;
    tokio::spawn(async move {
        run(&workspace_dir, &bound, file, plans, batch_turns).await;
        release(&workspace_dir);
    });
    Ok(view)
}

fn release(workspace_dir: &Path) {
    RUNNING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(workspace_dir);
}

async fn run(
    workspace_dir: &Path,
    bound: &BoundEngine,
    mut file: BackfillFile,
    plans: Vec<ThreadPlan>,
    batch_turns: u32,
) {
    for plan in plans {
        let cut = batches(
            &plan.thread_id,
            &plan.turns,
            plan.range.clone(),
            batch_turns,
        );
        for group in cut.chunks(STORE_GROUP) {
            let turns: u64 = group.iter().map(|batch| batch.turns.len() as u64).sum();
            let end = group.last().map_or(0, |batch| batch.last() + 1);
            let items: Vec<_> = group
                .iter()
                .cloned()
                .map(|batch| {
                    let mut item = batch.into_item();
                    item.meta_mut().tags.push(BACKFILL_TAG.to_string());
                    item
                })
                .collect();
            let count = items.len() as u64;
            if let Err(error) = store_many_on(bound, items).await {
                tracing::warn!(
                    code = error.code(),
                    "[memory:backfill] storing a batch failed; stopping"
                );
                file.state.phase = ImportPhase::Error;
                file.state.error = Some(String::from(error));
                file.state.finished_at = Some(Utc::now());
                write_file(workspace_dir, &file);
                return;
            }
            file.stored.insert(plan.thread_id.clone(), end);
            file.state.turns_stored += turns;
            file.state.items_stored += count;
            write_file(workspace_dir, &file);
        }
        file.state.threads_done += 1;
        write_file(workspace_dir, &file);
    }
    file.state.phase = ImportPhase::Done;
    file.state.finished_at = Some(Utc::now());
    write_file(workspace_dir, &file);
    tracing::info!(
        items = file.state.items_stored,
        turns = file.state.turns_stored,
        "[memory:backfill] done"
    );
}

#[cfg(test)]
#[path = "backfill_tests.rs"]
mod tests;
