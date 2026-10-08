//! Durable event journals + status stores for tinyagents turns (issue #4249,
//! Workstream 05-events, 05.1).
//!
//! The live [`crate::agent::tinyagents::observability::OpenhumanEventBridge`]
//! mirrors the harness [`EventSink`] onto openhuman's in-process `AgentProgress`
//! stream — transient state that is lost the moment the UI detaches. This module
//! makes that history **durable**: it attaches, *in addition to* the untouched
//! bridge, a crate [`StoreEventJournal`] (over the same 04-sessions
//! [`JsonlAppendStore`] under `{workspace}/tinyagents_store/journal`) plus a
//! [`HarnessStatusStore`] writer, so a run can be reconstructed after the fact —
//! even for an unobserved (`on_progress = None`) turn.
//!
//! Everything here is **best-effort and non-fatal**: opening the store,
//! subscribing the sink, and every status/journal write swallow errors behind a
//! grep-friendly `[journal]` log line and never fail or alter a chat turn. The
//! existing bridge/global-bus path is left fully intact — this is a pure
//! observer add-on.
//!
//! ## Composition
//!
//! The crate [`EventSink`] is itself the fan-out point: the (already-subscribed)
//! `OpenhumanEventBridge` and this journal sink are independent subscribers, so
//! **both** receive every event. The journal side is wrapped in a
//! [`FanOutSink`] as the durable-observer composition seam (05.2 will add graph
//! sinks here) and its records pass through a [`RedactingSink`] so process
//! credentials are masked before anything is persisted.
//!
//! ## Stable event ids (05.1)
//!
//! The run [`EventSink`] is seeded by the caller with
//! [`EventSink::with_stream_id`]`(run_id)` (see `tinyagents_harness::observability::mint_run_id`), so every
//! persisted observation carries a restart-stable `event_id` of the form
//! `{run_id}-evt-{offset}`. That is the id a late-attaching replay reader
//! reconstructs the timeline from — the same `(stream_id, offset)` always mints
//! the same id, and two runs never collide even if both restart their offset
//! counter at zero.
//!
//! ## Follow-ups (not in this slice)
//!
//! - Full sub-agent / graph run lineage (`parent_run_id` / `root_run_id`
//!   threading) — wired in 05.2/05.3. This slice threads `thread_id` (from the
//!   sub-agent task scope) so `FileStatusStore::list_by_thread` answers.
//!
//! [`EventSink::with_stream_id`]: tinyagents_harness::events::EventSink::with_stream_id

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};


use tinyagents_harness::events::{EventSink, HarnessRunStatus};
use tinyagents_harness::ids::{ComponentId, HarnessPhase, RunId, ThreadId};
use tinyagents_harness::observability::{
    process_env_secrets, AgentObservation, FanOutSink, FileStatusStore, HarnessEventJournal,
    HarnessStatusStore, JournalSink, RedactingSink, StoreEventJournal,
};

use tinyagents_harness::store::{AppendStore, Store};
use tinyagents_session::transcript::import::ops::open_session_stores;

/// Best-effort live request → durable tinyagents journal stream map. The web
/// progress bridge uses this at turn end to shadow-project spans from the
/// journal and compare them against the live `SpanCollector` path during C4 S3.
fn request_journal_runs() -> std::sync::Arc<RequestJournalRuns> {
    crate::core::runtime::current_slot::<RequestJournalRuns>()
}

type RequestJournalRuns = Mutex<HashMap<String, String>>;

pub(crate) fn register_request_journal_run(request_id: &str, run_id: &str) {
    match request_journal_runs().lock() {
        Ok(mut runs) => {
            runs.insert(request_id.to_string(), run_id.to_string());
            log::debug!(
                "[journal] registered request journal request_id={} run_id={}",
                request_id,
                run_id
            );
        }
        Err(err) => {
            log::debug!(
                "[journal] request journal registry poisoned request_id={} err={err}",
                request_id
            );
        }
    }
}

pub(crate) fn take_request_journal_run(request_id: &str) -> Option<String> {
    request_journal_runs()
        .lock()
        .ok()
        .and_then(|mut runs| runs.remove(request_id))
}

/// Resolve the internal workspace directory (`{workspace}`) whose
/// `tinyagents_store/` subtree holds the journal + kv stores. Async because the
/// config load is async; errors are surfaced so callers can log-and-skip.
async fn resolve_workspace() -> anyhow::Result<PathBuf> {
    let config = crate::config::ops::load_current_or_init()
        .await
        .map_err(|e| anyhow::anyhow!("[journal] load config for workspace: {e}"))?;
    Ok(config.workspace_dir)
}

/// The journal and key-value stores a turn's events and status go to: the
/// current agent's host session store when one is installed
/// ([`crate::agent::session_store::current`]), else the workspace's
/// `tinyagents_store/`.
async fn journal_stores() -> anyhow::Result<(Arc<dyn AppendStore>, Arc<dyn Store>)> {
    if let Some(stores) = crate::agent::session_store::current() {
        log::debug!("[journal] using the host session store");
        return Ok((stores.journal, stores.kv));
    }
    let workspace = resolve_workspace().await?;
    let stores = open_session_stores(&workspace);
    Ok((Arc::new(stores.journal), Arc::new(stores.kv)))
}

/// A live handle to a turn's durable journal + status snapshot.
///
/// Held by the turn loop for the duration of the run so it can stamp a terminal
/// status (`completed` / `failed`) once the harness returns — the harness
/// `AgentEvent` stream carries no run-terminal event, so the authoritative
/// terminal write is caller-driven here. Every method is best-effort and
/// non-fatal.
pub(crate) struct TurnJournal {
    run_id: RunId,
    status_store: Arc<FileStatusStore>,
    event_sink: Arc<JournalSink>,
    /// The in-flight status snapshot, mutated in place to `completed`/`failed`.
    status: Mutex<HarnessRunStatus>,
}

impl TurnJournal {
    /// Best-effort terminal write: mark the run completed and persist. Non-fatal.
    pub(crate) async fn finish_completed(&self) {
        self.event_sink.flush();
        let snapshot = {
            let mut guard = self.status.lock().unwrap();
            guard.mark_completed();
            guard.clone()
        };
        match self.status_store.put_status(snapshot).await {
            Ok(()) => log::debug!("[journal] run completed run_id={}", self.run_id.as_str()),
            Err(err) => log::debug!(
                "[journal] completed status write failed run_id={} err={err}",
                self.run_id.as_str()
            ),
        }
    }

    /// Best-effort terminal write: mark the run failed (recording `error`) and
    /// persist. Non-fatal.
    pub(crate) async fn finish_failed(&self, error: &str) {
        self.event_sink.flush();
        let snapshot = {
            let mut guard = self.status.lock().unwrap();
            guard.mark_failed(error);
            guard.clone()
        };
        match self.status_store.put_status(snapshot).await {
            Ok(()) => log::warn!(
                "[journal] run failed run_id={} error={error}",
                self.run_id.as_str()
            ),
            Err(err) => log::debug!(
                "[journal] failed status write failed run_id={} err={err}",
                self.run_id.as_str()
            ),
        }
    }
}

/// Attach a durable event journal + status writer to `events`, *in addition to*
/// the existing (untouched) [`OpenhumanEventBridge`] subscription.
///
/// `run_id` MUST be the same id the caller passed to
/// [`EventSink::with_stream_id`] when it created `events` (mint it once via
/// `tinyagents_harness::observability::mint_run_id`). That shared id is what makes the persisted `event_id`s the
/// restart-stable `{run_id}-evt-{offset}` a late-attach replay reconstructs the
/// timeline from. `thread_id` (when known — e.g. the sub-agent task scope)
/// records the run under a thread so `FileStatusStore::list_by_thread` answers.
///
/// Returns a [`TurnJournal`] handle the caller uses to stamp the terminal
/// status after the run, or `None` when the store could not be opened (the run
/// proceeds unaffected — journaling is best-effort). Safe to call for observed
/// and unobserved turns alike: it does not depend on `on_progress`.
///
/// [`OpenhumanEventBridge`]: crate::agent::tinyagents::observability::OpenhumanEventBridge
/// [`EventSink::with_stream_id`]: tinyagents_harness::events::EventSink::with_stream_id
pub(crate) async fn attach_turn_journal(
    events: &EventSink,
    model: &str,
    run_id: RunId,
    thread_id: Option<ThreadId>,
) -> Option<TurnJournal> {
    let (journal_store, kv) = match journal_stores().await {
        Ok(stores) => stores,
        Err(err) => {
            log::debug!("[journal] skipping journal attach; {err}");
            return None;
        }
    };

    // Event journal: crate StoreEventJournal over the 04-sessions JsonlAppendStore
    // (stream key = run id). Wrapped in a JournalSink (stamps run lineage) and a
    // RedactingSink (masks process credentials) before persisting. Because
    // `events` was seeded with `with_stream_id(run_id)`, every persisted
    // observation's `event_id` is the stable `{run_id}-evt-{offset}`.
    let journal: Arc<dyn HarnessEventJournal> = Arc::new(StoreEventJournal::new(journal_store));
    let journal_sink = Arc::new(JournalSink::new(journal, run_id.clone()));
    let redacting = RedactingSink::new(journal_sink.clone(), process_env_secrets());

    // FanOutSink is the durable-observer composition seam (05.2 adds graph sinks
    // here). Subscribing it as its own listener leaves the bridge subscription
    // untouched — the EventSink fans out to both.
    let fanout = FanOutSink::new().with(Arc::new(redacting));
    events.subscribe(Arc::new(fanout));

    // Status store: durable, Store-backed. Seed an initial `running` snapshot,
    // recording the thread (when known) so list_by_thread answers at run start.
    let status_store = Arc::new(FileStatusStore::over(kv));
    let mut status = HarnessRunStatus::new(run_id.clone(), ComponentId::new(model.to_string()));
    if let Some(thread_id) = thread_id {
        status = status.with_thread(thread_id);
    }
    status.mark_running(HarnessPhase::Model);
    if let Err(err) = status_store.put_status(status.clone()).await {
        log::debug!(
            "[journal] initial status write failed run_id={} err={err}",
            run_id.as_str()
        );
    }

    log::debug!(
        "[journal] attached durable event journal run_id={} thread={:?} model={model}",
        run_id.as_str(),
        status.thread_id.as_ref().map(|t| t.as_str())
    );
    Some(TurnJournal {
        run_id,
        status_store,
        event_sink: journal_sink,
        status: Mutex::new(status),
    })
}

/// Late-attach replay reader: return every persisted observation for `run_id`
/// whose stream offset is `>= from_offset`, in order. Reading from `0` replays
/// the whole run.
///
/// This is the seam a future replay RPC (05.x) will call so the desktop can
/// reconnect mid-run and backfill the timeline from durable state instead of
/// relying on transient `AgentProgress` buffering. Best-effort: a missing store
/// or unknown run yields an empty `Vec`, not an error.
pub(crate) async fn read_run_events(
    run_id: &str,
    from_offset: u64,
) -> anyhow::Result<Vec<AgentObservation>> {
    let (journal_store, _) = journal_stores().await?;
    let journal = StoreEventJournal::new(journal_store);
    journal
        .read_from(run_id, from_offset)
        .await
        .map_err(|e| anyhow::anyhow!("[journal] read_run_events failed run_id={run_id}: {e}"))
}
