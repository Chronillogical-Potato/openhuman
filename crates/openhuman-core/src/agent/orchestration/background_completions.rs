//! Host adapter over the harness `CompletionRouter` for finished detached
//! background sub-agents (`spawn_async_subagent`).
//!
//! The queue itself is the harness's: `tinyagents_tasks::CompletionRouter`
//! deduplicates by task id, holds tombstones (collected inline, stopped,
//! cancelled parent), counts delivery attempts, hands back the records that
//! gave up, and persists every transition to a per-workspace
//! `JsonlCompletionStore`, so a completion that was never delivered is
//! delivered after a restart. This module is the thin host seam over it:
//!
//! * one router per workspace, opened lazily next to the task ledger
//!   (`<workspace>/.openhuman/background_completions.jsonl`);
//! * the router's parent key is the **chat thread id** — delivery is
//!   thread-addressed, and a thread-scoped cancel (Stop, delete, purge) is then
//!   the router's own `cancel_parent` / `resume_parent`;
//! * the product-specific lifecycle helpers (`record_failure`,
//!   `record_awaiting_input`, `mark_collected`, ...) the spawn, wait, cancel and
//!   thread paths call.
//!
//! When a parent is idle enough to receive a delivery turn, and the turn itself,
//! stays in [`super::background_delivery`]. The wording is
//! [`super::completion_notice`].

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use tinyagents_tasks::{
    CompletionRecord, CompletionResult, CompletionRouter, CompletionState, CompletionStore,
    InMemoryCompletionStore, JsonlCompletionStore, NotifyMode, RecordOutcome, TombstoneOutcome,
    DEFAULT_MAX_ATTEMPTS,
};

pub(crate) use super::completion_notice::BackgroundAgentOutcome;
use super::completion_notice::BackgroundCompletionFormatter;

/// How long a settled record (delivered / gave up / tombstoned) is kept before
/// compaction drops it. Dropping a settled record also drops its dedupe and its
/// tombstone, so this must outlive any child's interest in its parent.
const SETTLED_RETENTION: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// Retries for a failed store write before a completion is reported lost.
const RECORD_RETRIES: u32 = 3;

/// Bound on the session -> thread cache. Entries are cheap and re-learned, so
/// overflow simply clears it.
const SESSION_THREADS_CAP: usize = 4096;

/// A workspace's router plus the store it sits on (kept so boot recovery and
/// purge can enumerate parents, which the router itself does not expose).
struct Entry {
    router: Arc<CompletionRouter>,
    store: Arc<dyn CompletionStore>,
}

#[derive(Default)]
struct HostState {
    /// One router per workspace.
    routers: HashMap<PathBuf, Arc<Entry>>,
    /// Which workspace holds a thread's completions. Thread-scoped operations
    /// (Stop, delete) carry no workspace, only a thread id.
    thread_workspaces: HashMap<String, PathBuf>,
    /// Parent session id -> chat thread id, for the idle gate (busy is tracked
    /// by session; delivery by thread).
    session_threads: HashMap<String, String>,
    /// Threads the user stopped and has not yet re-engaged. Closes the
    /// spawn/register race: a child that registers after Stop is rejected.
    stopped_threads: HashSet<String>,
}

fn state() -> std::sync::MutexGuard<'static, HostState> {
    static STATE: OnceLock<Mutex<HostState>> = OnceLock::new();
    STATE
        .get_or_init(|| Mutex::new(HostState::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Where a workspace's completion log lives, beside the detached-task ledger.
fn completion_store_path(workspace_dir: &Path) -> PathBuf {
    workspace_dir
        .join(".openhuman")
        .join("background_completions.jsonl")
}

fn entry_for(workspace_dir: &Path) -> Arc<Entry> {
    let mut st = state();
    if let Some(entry) = st.routers.get(workspace_dir) {
        return entry.clone();
    }
    let path = completion_store_path(workspace_dir);
    let store: Arc<dyn CompletionStore> = match JsonlCompletionStore::open(&path) {
        Ok(store) => Arc::new(store),
        Err(error) => {
            // A workspace that cannot be written degrades to an in-memory
            // queue rather than taking background delivery down.
            log::warn!(
                "[background_completions] could not open {}; using an in-memory queue \
                 (completions will not survive a restart): {error}",
                path.display()
            );
            Arc::new(InMemoryCompletionStore::new())
        }
    };
    let router = CompletionRouter::new(store.clone())
        .with_formatter(Arc::new(BackgroundCompletionFormatter))
        .with_max_attempts(DEFAULT_MAX_ATTEMPTS);
    let entry = Arc::new(Entry {
        router: Arc::new(router),
        store,
    });
    st.routers
        .insert(workspace_dir.to_path_buf(), entry.clone());
    log::debug!(
        "[background_completions] opened router workspace_dir={}",
        workspace_dir.display()
    );
    entry
}

/// The router for `workspace_dir`, opening it on first use.
pub(crate) fn router_for_workspace(workspace_dir: &Path) -> Arc<CompletionRouter> {
    entry_for(workspace_dir).router.clone()
}

/// Remember which workspace holds `thread_id`'s completions.
fn note_thread_workspace(thread_id: &str, workspace_dir: &Path) {
    state()
        .thread_workspaces
        .insert(thread_id.to_string(), workspace_dir.to_path_buf());
}

/// The router that holds `thread_id`'s completions, if any child of that thread
/// was spawned or recorded in this process.
pub(crate) fn router_for_thread(thread_id: &str) -> Option<Arc<CompletionRouter>> {
    let workspace = state().thread_workspaces.get(thread_id).cloned()?;
    Some(router_for_workspace(&workspace))
}

/// Remember that `session_id` is a turn on `thread_id`.
pub(crate) fn note_session_thread(session_id: &str, thread_id: &str) {
    let mut st = state();
    if st.session_threads.len() >= SESSION_THREADS_CAP {
        st.session_threads.clear();
    }
    st.session_threads
        .insert(session_id.to_string(), thread_id.to_string());
}

/// The chat thread a session id belongs to: the cached mapping, else the
/// `thread_id` a web-channel session id carries in its JSON body.
pub(crate) fn thread_for_session(session_id: &str) -> Option<String> {
    if let Some(thread) = state().session_threads.get(session_id) {
        return Some(thread.clone());
    }
    if !session_id.starts_with('{') {
        return None;
    }
    serde_json::from_str::<serde_json::Value>(session_id)
        .ok()?
        .get("thread_id")?
        .as_str()
        .map(str::to_owned)
}

/// Record a finished background sub-agent for idle delivery, keyed by its
/// parent chat thread. Idempotent on `task_id`.
pub(crate) async fn record_completion(
    workspace_dir: &Path,
    parent_session: &str,
    task_id: impl Into<String>,
    agent_id: impl Into<String>,
    summary: impl Into<String>,
    parent_thread_id: Option<String>,
) {
    record_outcome(
        workspace_dir,
        parent_session,
        task_id,
        agent_id,
        summary,
        parent_thread_id,
        BackgroundAgentOutcome::Completed,
    )
    .await;
}

/// Record a finished background sub-agent carrying an explicit terminal
/// [`BackgroundAgentOutcome`]. The general enqueue behind
/// [`record_completion`] and the [`record_failure`] / [`record_awaiting_input`]
/// framing helpers, so a failed or awaiting-input async sub-agent is delivered
/// back into chat too — not only successes (#4896).
///
/// The router drops the record when its task was tombstoned (collected inline,
/// stopped) or its thread was cancelled (deleted, stopped).
pub(crate) async fn record_outcome(
    workspace_dir: &Path,
    parent_session: &str,
    task_id: impl Into<String>,
    agent_id: impl Into<String>,
    summary: impl Into<String>,
    parent_thread_id: Option<String>,
    outcome: BackgroundAgentOutcome,
) {
    let task_id = task_id.into();
    let Some(thread_id) = parent_thread_id else {
        // Delivery is thread-addressed; a headless spawn has nowhere to land
        // the result. (`spawn_async_subagent` refuses to start one.)
        log::warn!(
            "[background_completions] dropping headless completion task_id={task_id} \
             session={parent_session}"
        );
        return;
    };
    note_thread_workspace(&thread_id, workspace_dir);
    note_session_thread(parent_session, &thread_id);

    let record = CompletionRecord::new(
        task_id.clone(),
        thread_id.clone(),
        agent_id,
        outcome.status(),
        CompletionResult::text(summary),
    )
    .with_notify_mode(NotifyMode::Followup);
    let router = router_for_workspace(workspace_dir);
    match router.record_with_retries(record, RECORD_RETRIES).await {
        Ok(RecordOutcome::Recorded { .. }) => log::debug!(
            "[background_completions] recorded task_id={task_id} thread_id={thread_id} \
             outcome={outcome:?}"
        ),
        Ok(RecordOutcome::Duplicate) => {
            log::debug!("[background_completions] duplicate ignored task_id={task_id}")
        }
        Ok(RecordOutcome::Suppressed) => log::debug!(
            "[background_completions] dropping completion task_id={task_id} for \
             stopped/cancelled/collected thread_id={thread_id}"
        ),
        Ok(RecordOutcome::IdCollision) => log::warn!(
            "[background_completions] task id already recorded for another thread \
             task_id={task_id} thread_id={thread_id}"
        ),
        Err(error) => log::error!(
            "[background_completions] completion LOST — store write failed \
             task_id={task_id} thread_id={thread_id} error={error}"
        ),
    }
}

/// Queue a **failed** async sub-agent for chat delivery (#4896). The summary is
/// framed with the `[SUBAGENT_FAILED]` envelope the parent agent is prompted to
/// relay, so the user learns the delegated task errored instead of the turn
/// silently finalizing on "Accepted".
pub(crate) async fn record_failure(
    workspace_dir: &Path,
    parent_session: &str,
    task_id: impl Into<String>,
    agent_id: impl Into<String>,
    error: &str,
    parent_thread_id: Option<String>,
) {
    let summary =
        format!("[SUBAGENT_FAILED] the async sub-agent errored before producing a result: {error}");
    record_outcome(
        workspace_dir,
        parent_session,
        task_id,
        agent_id,
        summary,
        parent_thread_id,
        BackgroundAgentOutcome::Failed,
    )
    .await;
}

/// Queue an **awaiting-user** async sub-agent for chat delivery (#4896). A
/// detached child that pauses to ask a question will not continue on its own, so
/// the framed `[SUBAGENT_NEEDS_INPUT]` notice is delivered back into chat for the
/// parent agent to relay to (or answer for) the user.
pub(crate) async fn record_awaiting_input(
    workspace_dir: &Path,
    parent_session: &str,
    task_id: impl Into<String>,
    agent_id: impl Into<String>,
    question: &str,
    checkpointed: bool,
    parent_thread_id: Option<String>,
) {
    let task_id = task_id.into();
    let agent_id = agent_id.into();
    let summary = crate::agent::orchestration::tools::awaiting_user::awaiting_user_envelope(
        &task_id,
        &agent_id,
        None,
        question,
        checkpointed,
    );
    record_outcome(
        workspace_dir,
        parent_session,
        task_id,
        agent_id,
        summary,
        parent_thread_id,
        BackgroundAgentOutcome::AwaitingInput,
    )
    .await;
}

/// Where one detached child's completion lands: its workspace (which router),
/// the parent session (the idle gate) and the parent chat thread (the router's
/// parent key). Built once at spawn so every terminal path records the same way.
#[derive(Clone, Debug)]
pub(crate) struct CompletionTarget {
    workspace_dir: PathBuf,
    parent_session: String,
    parent_thread_id: Option<String>,
}

impl CompletionTarget {
    pub(crate) fn new(
        workspace_dir: PathBuf,
        parent_session: String,
        parent_thread_id: Option<String>,
    ) -> Self {
        Self {
            workspace_dir,
            parent_session,
            parent_thread_id,
        }
    }

    /// Queue a finished result. See [`record_completion`].
    pub(crate) async fn completed(&self, task_id: &str, agent_id: &str, summary: String) {
        record_completion(
            &self.workspace_dir,
            &self.parent_session,
            task_id,
            agent_id,
            summary,
            self.parent_thread_id.clone(),
        )
        .await;
    }

    /// Queue a failure. See [`record_failure`].
    pub(crate) async fn failed(&self, task_id: &str, agent_id: &str, error: &str) {
        record_failure(
            &self.workspace_dir,
            &self.parent_session,
            task_id,
            agent_id,
            error,
            self.parent_thread_id.clone(),
        )
        .await;
    }

    /// Queue an awaiting-input pause. See [`record_awaiting_input`].
    pub(crate) async fn awaiting_input(
        &self,
        task_id: &str,
        agent_id: &str,
        question: &str,
        checkpointed: bool,
    ) {
        record_awaiting_input(
            &self.workspace_dir,
            &self.parent_session,
            task_id,
            agent_id,
            question,
            checkpointed,
            self.parent_thread_id.clone(),
        )
        .await;
    }
}

/// Undelivered completions for `thread_id`, oldest first (read-only; includes
/// records an in-flight delivery currently holds).
pub(crate) fn pending_for(workspace_dir: &Path, thread_id: &str) -> Vec<CompletionRecord> {
    router_for_workspace(workspace_dir).pending_for(thread_id)
}

/// Mark `task_id` as collected inline by the parent (via `wait_subagent`) so its
/// background completion is not independently delivered as a second, duplicate
/// answer. The router withdraws an already-recorded completion and drops one
/// that arrives later. Returns whether a pending completion was withdrawn.
pub(crate) fn mark_collected(workspace_dir: &Path, task_id: &str) -> bool {
    let outcome = router_for_workspace(workspace_dir).tombstone(task_id);
    log::debug!("[background_completions] mark_collected task_id={task_id} outcome={outcome:?}");
    matches!(outcome, Ok(TombstoneOutcome::Suppressed))
}

/// Drop every queued completion for `thread_id` and everything that finishes
/// for it later. Called when the thread is deleted: the router's cancelled-parent
/// marker is durable, so a straggler that wins the cooperative-abort race — or
/// finishes after a restart — is dropped rather than delivered into a thread that
/// no longer exists. Returns the number of queued completions removed.
pub(crate) fn discard_for_thread(thread_id: &str) -> usize {
    let Some(router) = router_for_thread(thread_id) else {
        return 0;
    };
    let removed = router.cancel_parent(thread_id).unwrap_or_else(|error| {
        log::error!(
            "[background_completions] cancel_parent failed thread_id={thread_id} error={error}"
        );
        0
    });
    let mut st = state();
    st.thread_workspaces.remove(thread_id);
    st.stopped_threads.remove(thread_id);
    log::debug!(
        "[background_completions] discard_for_thread thread_id={thread_id} removed={removed}"
    );
    removed
}

/// Drop every queued completion for `thread_id` and gate late results from the
/// stopped generation.
///
/// The Stop-button counterpart of [`discard_for_thread`]: the user halted the
/// thread's work, so results that finished but were not yet delivered must not
/// start a fresh delivery turn behind their back. The thread stays alive; the
/// gate lifts at [`resume_stopped_thread`] (the next accepted user turn), while
/// the stopped generation's task ids stay tombstoned
/// ([`finish_stop_for_thread`]). Returns the number of queued completions removed.
pub(crate) fn discard_pending_for_thread(thread_id: &str) -> usize {
    state().stopped_threads.insert(thread_id.to_string());
    let Some(router) = router_for_thread(thread_id) else {
        return 0;
    };
    let removed = router.cancel_parent(thread_id).unwrap_or_else(|error| {
        log::error!(
            "[background_completions] cancel_parent failed thread_id={thread_id} error={error}"
        );
        0
    });
    log::debug!(
        "[background_completions] discard_pending_for_thread thread_id={thread_id} removed={removed}"
    );
    removed
}

/// Complete a Stop after its registered children were aborted: tombstone their
/// task ids so a straggler from the stopped generation cannot record once a
/// later user turn reopens the thread.
pub(crate) fn finish_stop_for_thread(thread_id: &str, task_ids: &[String]) {
    let Some(router) = router_for_thread(thread_id) else {
        return;
    };
    for task_id in task_ids {
        if let Err(error) = router.tombstone(task_id) {
            log::warn!("[background_completions] tombstone failed task_id={task_id} error={error}");
        }
    }
}

/// Reopen a thread's completion gate for a newly accepted user turn.
///
/// The Stop gate deliberately outlives registry cancellation, because a detached
/// child may be between `tokio::spawn` and `running_subagents::register` when
/// Stop is pressed. New task ids remain distinct from the stopped generation.
/// Only a thread this process stopped or cancelled pays for the durable resume
/// marker, so an ordinary chat message writes nothing.
pub(crate) fn resume_stopped_thread(thread_id: &str) {
    let was_stopped = state().stopped_threads.remove(thread_id);
    if !was_stopped {
        return;
    }
    if let Some(router) = router_for_thread(thread_id) {
        router.resume_parent(thread_id);
        log::debug!("[background_completions] resumed thread_id={thread_id}");
    }
}

/// Record a child that registers while its parent thread is stopped.
///
/// Registration happens after the detached task is spawned. If Stop races that
/// narrow interval the registry sweep cannot see the child; tombstoning its task
/// id here keeps it rejected even after a later user turn reopens the thread.
/// Also notes which workspace holds the thread's completions, so a later
/// thread-scoped Stop or delete can find the router. The first child this
/// process spawns on a thread lifts any cancelled-parent marker an earlier
/// process left behind (a Stop before a restart): a live spawn proves the user
/// re-engaged the thread. Returns whether the thread is stopped.
pub(crate) fn mark_stopped_task_if_thread_stopped(
    workspace_dir: &Path,
    thread_id: &str,
    task_id: &str,
) -> bool {
    let (first_sight, stopped) = {
        let mut st = state();
        let first_sight = st
            .thread_workspaces
            .insert(thread_id.to_string(), workspace_dir.to_path_buf())
            .is_none();
        (first_sight, st.stopped_threads.contains(thread_id))
    };
    let router = router_for_workspace(workspace_dir);
    if stopped {
        if let Err(error) = router.tombstone(task_id) {
            log::warn!("[background_completions] tombstone failed task_id={task_id} error={error}");
        }
        return true;
    }
    if first_sight {
        router.resume_parent(thread_id);
    }
    false
}

/// Withdraw every queued completion across all open workspaces. Called on a full
/// thread purge; each thread with undelivered results is cancelled durably, so
/// stragglers are still dropped. Returns the number of completions removed.
pub(crate) fn clear_all() -> usize {
    let entries: Vec<Arc<Entry>> = state().routers.values().cloned().collect();
    let mut removed = 0;
    for entry in entries {
        let parents: HashSet<String> = entry
            .store
            .list(None)
            .into_iter()
            .filter(|r| r.state == CompletionState::Pending && !r.parent_key.is_empty())
            .map(|r| r.parent_key)
            .collect();
        for parent in parents {
            removed += entry.router.cancel_parent(&parent).unwrap_or(0);
        }
    }
    log::debug!("[background_completions] clear_all removed={removed}");
    removed
}

/// Boot recovery for a workspace: open its log, drop long-settled records, and
/// return the threads that still hold undelivered completions (a previous
/// process finished the work but never delivered it). Remembers each thread's
/// workspace so the delivery loop can claim them.
pub(crate) fn recover_pending_threads(workspace_dir: &Path) -> Vec<String> {
    let entry = entry_for(workspace_dir);
    if let Err(error) = entry.router.compact(SETTLED_RETENTION) {
        log::warn!("[background_completions] compact failed error={error}");
    }
    let threads: HashSet<String> = entry
        .store
        .list(None)
        .into_iter()
        .filter(|r| {
            r.state == CompletionState::Pending
                && r.notify_mode != NotifyMode::Off
                && !r.parent_key.is_empty()
        })
        .map(|r| r.parent_key)
        .collect();
    let mut threads: Vec<String> = threads
        .into_iter()
        // `pending_for` is the router's view: it drops threads a cancelled-parent
        // marker has since withdrawn.
        .filter(|thread| !entry.router.pending_for(thread).is_empty())
        .collect();
    threads.sort();
    for thread in &threads {
        note_thread_workspace(thread, workspace_dir);
    }
    log::info!(
        "[background_completions] boot recovery found {} thread(s) with undelivered completions \
         workspace_dir={}",
        threads.len(),
        workspace_dir.display()
    );
    threads
}

/// Forget everything this process knows about `workspace_dir`, as a restart
/// would: the router (and its open log handle) and every thread/session mapping
/// that points at it. The on-disk log is untouched.
#[cfg(test)]
pub(crate) fn forget_workspace_for_test(workspace_dir: &Path) {
    let mut st = state();
    st.routers.remove(workspace_dir);
    let gone: Vec<String> = st
        .thread_workspaces
        .iter()
        .filter(|(_, ws)| ws.as_path() == workspace_dir)
        .map(|(thread, _)| thread.clone())
        .collect();
    for thread in gone {
        st.thread_workspaces.remove(&thread);
        st.stopped_threads.remove(&thread);
    }
    st.session_threads.clear();
}

#[cfg(test)]
#[path = "background_completions_tests.rs"]
mod tests;
