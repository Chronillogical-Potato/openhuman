use super::*;
use crate::memory::error::{INVALID_REQUEST, MEMORY_OFF};
use crate::memory::test_fixtures::{bind_reference, config_in, stored};
use std::sync::Arc;

use rusqlite::{params, Connection};
use tinymemory_api::MetaFilter;

/// An early v1 `memory.db` (no optional columns), enough for the importer.
const LEGACY_DDL: &str = "
CREATE TABLE memory_docs (
  document_id TEXT PRIMARY KEY, namespace TEXT NOT NULL, key TEXT NOT NULL, title TEXT NOT NULL,
  content TEXT NOT NULL, source_type TEXT NOT NULL, priority TEXT NOT NULL, tags_json TEXT NOT NULL,
  metadata_json TEXT NOT NULL, category TEXT NOT NULL, session_id TEXT, created_at REAL NOT NULL,
  updated_at REAL NOT NULL, markdown_rel_path TEXT NOT NULL, UNIQUE(namespace, key));
CREATE TABLE episodic_log (id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL, timestamp REAL NOT NULL,
  role TEXT NOT NULL, content TEXT NOT NULL, lesson TEXT);
CREATE TABLE user_profile (facet_id TEXT PRIMARY KEY, facet_type TEXT NOT NULL, key TEXT NOT NULL, value TEXT NOT NULL,
  confidence REAL NOT NULL DEFAULT 0.5, evidence_count INTEGER NOT NULL DEFAULT 1, source_segment_ids TEXT,
  first_seen_at REAL NOT NULL, last_seen_at REAL NOT NULL);
";

fn memory_doc(conn: &Connection, id: &str, namespace: &str, title: &str, content: &str) {
    conn.execute(
        "INSERT INTO memory_docs (document_id, namespace, key, title, content, source_type, priority,
           tags_json, metadata_json, category, created_at, updated_at, markdown_rel_path)
         VALUES (?1, ?2, ?1, ?3, ?4, 'chat', 'normal', '[]', '{}', 'core', 1700000000.0, 1700000000.0, '')",
        params![id, namespace, title, content],
    )
    .unwrap();
}

/// A v1 workspace: two documents, one learning, one conversation, one facet.
fn legacy_workspace(workspace_dir: &Path) {
    std::fs::create_dir_all(workspace_dir.join("memory")).unwrap();
    let conn = Connection::open(workspace_dir.join("memory").join("memory.db")).unwrap();
    conn.execute_batch(LEGACY_DDL).unwrap();
    memory_doc(&conn, "d1", "notes", "Plan", "Ship memory v2 on Friday");
    memory_doc(&conn, "d2", "notes", "Ideas", "Try oolong tea");
    memory_doc(&conn, "d3", "learning:style", "Style", "Keep answers terse");
    for (n, (role, text)) in [("user", "hi there"), ("assistant", "hello")]
        .iter()
        .enumerate()
    {
        conn.execute(
            "INSERT INTO episodic_log (session_id, timestamp, role, content) VALUES ('s1', ?1, ?2, ?3)",
            params![1_700_000_000.0 + n as f64, role, text],
        )
        .unwrap();
    }
    conn.execute(
        "INSERT INTO user_profile (facet_id, facet_type, key, value, confidence, first_seen_at, last_seen_at)
         VALUES ('f1', 'preference', 'tone', 'terse', 0.9, 1700000000.0, 1700000000.0)",
        [],
    )
    .unwrap();
}

async fn wait_until_settled(config: &Config) -> ImportState {
    for _ in 0..400 {
        let state = status(config);
        if state.phase != ImportPhase::Running {
            return state;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    panic!("import never settled");
}

#[tokio::test]
async fn scan_reports_no_store_for_a_fresh_workspace() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    let view = scan(&config).await.unwrap();
    assert!(!view.found);
    assert!(view.counts.is_none());
    assert!(count_legacy(&config.workspace_dir).is_none());
}

#[tokio::test]
async fn scan_counts_what_a_legacy_store_holds() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    let view = scan(&config).await.unwrap();
    assert!(view.found);
    let counts = view.counts.expect("counts");
    assert_eq!(counts.documents, 2);
    assert_eq!(counts.conversations, 1);
    assert_eq!(counts.learnings, 2, "a learning doc and a profile facet");
}

#[test]
fn status_of_an_untouched_workspace_is_idle() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    assert_eq!(status(&config), ImportState::default());
}

#[tokio::test]
async fn start_is_refused_without_consent() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    bind_reference(&config);
    let error = start(&config, false).await.unwrap_err();
    assert_eq!(error.code(), INVALID_REQUEST);
    assert!(error.to_string().contains("consent"));
    assert_eq!(status(&config).phase, ImportPhase::Idle, "nothing started");
}

#[tokio::test]
async fn start_needs_memory_on_and_a_legacy_store() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    assert_eq!(start(&config, true).await.unwrap_err().code(), MEMORY_OFF);

    let tmp2 = tempfile::tempdir().unwrap();
    let empty = config_in(&tmp2);
    bind_reference(&empty);
    for _ in 0..2 {
        // The claim is released, so asking again gives the same answer rather
        // than a stale "already running" status.
        assert_eq!(
            start(&empty, true).await.unwrap_err().code(),
            INVALID_REQUEST
        );
    }
}

#[tokio::test]
async fn a_full_import_stores_every_item_and_finishes_done() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    let engine = bind_reference(&config);

    let started = start(&config, true).await.unwrap();
    assert_eq!(started.phase, ImportPhase::Running);
    assert_eq!(started.total, 5);

    let done = wait_until_settled(&config).await;
    assert_eq!(done.phase, ImportPhase::Done, "{done:?}");
    assert_eq!(done.imported, 5);
    assert_eq!(done.total, 5);
    assert!(done.error.is_none());
    let items = stored(&engine, MetaFilter::default()).await;
    assert_eq!(items.len(), 5);
    assert!(items
        .iter()
        .all(|item| item.meta.source.kind == tinymemory_api::SourceKind::Import));
    assert!(file_path(&config.workspace_dir).exists());
}

#[tokio::test]
async fn an_interrupted_import_resumes_from_its_checkpoint() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    let engine = bind_reference(&config);

    // A previous run stored d1 and stopped on an error.
    write_file(
        &config.workspace_dir,
        &ImportFile {
            paused_for_credits: false,
            state: ImportState {
                phase: ImportPhase::Error,
                imported: 1,
                total: 5,
                error: Some("unauthorized: sign in".into()),
            },
            checkpoint: Checkpoint {
                documents: Some("d1".into()),
                ..Checkpoint::default()
            },
        },
    );
    assert_eq!(status(&config).phase, ImportPhase::Error);

    start(&config, true).await.unwrap();
    let done = wait_until_settled(&config).await;
    assert_eq!(done.phase, ImportPhase::Done, "{done:?}");
    assert_eq!(done.imported, 5, "the earlier item counts toward the total");
    let items = stored(&engine, MetaFilter::default()).await;
    assert_eq!(items.len(), 4, "d1 is not sent again");
    assert!(
        !items
            .iter()
            .any(|item| item.text.contains("Ship memory v2")),
        "the checkpointed document was skipped"
    );
}

#[test]
fn a_running_state_with_no_live_import_reads_as_interrupted() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    write_file(
        &config.workspace_dir,
        &ImportFile {
            paused_for_credits: false,
            state: ImportState {
                phase: ImportPhase::Running,
                imported: 3,
                total: 9,
                error: None,
            },
            checkpoint: Checkpoint::default(),
        },
    );
    let state = status(&config);
    assert_eq!(state.phase, ImportPhase::Error);
    assert_eq!(state.imported, 3);
    assert!(state
        .error
        .as_deref()
        .unwrap()
        .contains("resumes on its own"));
}

#[tokio::test]
async fn a_finished_import_starts_over_when_run_again() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    bind_reference(&config);
    start(&config, true).await.unwrap();
    assert_eq!(wait_until_settled(&config).await.phase, ImportPhase::Done);

    let again = start(&config, true).await.unwrap();
    assert_eq!(again.imported, 0, "Done does not resume; it restarts");
    assert_eq!(wait_until_settled(&config).await.imported, 5);
}

#[tokio::test]
async fn a_second_start_while_running_returns_the_current_status() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    bind_reference(&config);
    RUNNING.lock().unwrap().insert(config.workspace_dir.clone());
    let state = start(&config, true).await.unwrap();
    assert_eq!(state.phase, ImportPhase::Idle, "no second run was launched");
    RUNNING.lock().unwrap().remove(&config.workspace_dir);
}

#[test]
fn a_corrupt_import_file_reads_as_idle() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("memory")).unwrap();
    std::fs::write(file_path(tmp.path()), "garbage").unwrap();
    assert_eq!(read_file(tmp.path()).state, ImportState::default());
}

/// A reference engine that refuses a write with whatever `refuse` returns for
/// its item, and stores it otherwise.
struct FailingEngine {
    inner: Arc<tinymemory_api::conformance::ReferenceEngine>,
    refuse: fn(&tinymemory_api::StoreItem) -> Option<tinymemory_api::Error>,
}

#[async_trait::async_trait]
impl tinymemory_api::MemoryEngine for FailingEngine {
    fn descriptor(&self) -> &tinymemory_api::EngineDescriptor {
        self.inner.descriptor()
    }
    async fn health(&self) -> tinymemory_api::EngineHealth {
        self.inner.health().await
    }
    async fn recall(
        &self,
        req: tinymemory_api::RecallRequest,
    ) -> tinymemory_api::Result<tinymemory_api::RecallAnswer> {
        self.inner.recall(req).await
    }
    async fn fetch(
        &self,
        req: tinymemory_api::FetchRequest,
    ) -> tinymemory_api::Result<tinymemory_api::FetchPage> {
        self.inner.fetch(req).await
    }
    async fn store(
        &self,
        item: tinymemory_api::StoreItem,
    ) -> tinymemory_api::Result<tinymemory_api::StoreReceipt> {
        if let Some(error) = (self.refuse)(&item) {
            return Err(error);
        }
        self.inner.store(item).await
    }
    async fn forget(
        &self,
        target: tinymemory_api::ForgetTarget,
    ) -> tinymemory_api::Result<tinymemory_api::ForgetReport> {
        self.inner.forget(target).await
    }
    async fn list(
        &self,
        req: tinymemory_api::ListRequest,
    ) -> tinymemory_api::Result<tinymemory_api::ListPage> {
        self.inner.list(req).await
    }
}

/// Binds a [`FailingEngine`] and returns the engine behind it.
fn bind_failing(
    config: &Config,
    refuse: fn(&tinymemory_api::StoreItem) -> Option<tinymemory_api::Error>,
) -> Arc<tinymemory_api::conformance::ReferenceEngine> {
    let inner = Arc::new(tinymemory_api::conformance::ReferenceEngine::new());
    crate::memory::engine::install_test_engine(
        &config.workspace_dir,
        Arc::new(FailingEngine {
            inner: inner.clone(),
            refuse,
        }),
    );
    inner
}

fn out_of_credits(_: &tinymemory_api::StoreItem) -> Option<tinymemory_api::Error> {
    Some(tinymemory_api::Error::Engine(
        "[USER_INSUFFICIENT_CREDITS] insufficient credits (HTTP 402)".into(),
    ))
}

#[tokio::test]
async fn exhausted_credits_stop_the_import_instead_of_skipping_everything() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    let engine = bind_failing(&config, out_of_credits);

    start(&config, true).await.unwrap();
    let state = wait_until_settled(&config).await;
    assert_eq!(state.phase, ImportPhase::Error, "{state:?}");
    assert_eq!(state.imported, 0);
    assert!(
        state
            .error
            .as_deref()
            .unwrap()
            .contains("not enough credits"),
        "{state:?}"
    );
    assert!(stored(&engine, MetaFilter::default()).await.is_empty());
    assert_eq!(
        read_file(&config.workspace_dir).checkpoint,
        Checkpoint::default(),
        "the checkpoint does not move past items that were never stored"
    );
}

#[tokio::test]
async fn an_unreachable_engine_stops_the_import_and_a_retry_resumes_it() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    bind_failing(&config, |_| {
        Some(tinymemory_api::Error::Unavailable(
            "connection refused".into(),
        ))
    });

    start(&config, true).await.unwrap();
    let state = wait_until_settled(&config).await;
    assert_eq!(state.phase, ImportPhase::Error, "{state:?}");
    assert_eq!(state.imported, 0);

    // The engine is back: starting again imports everything, nothing skipped.
    let engine = bind_reference(&config);
    start(&config, true).await.unwrap();
    let done = wait_until_settled(&config).await;
    assert_eq!(done.phase, ImportPhase::Done, "{done:?}");
    assert_eq!(stored(&engine, MetaFilter::default()).await.len(), 5);
}

#[tokio::test]
async fn an_item_the_engine_refuses_is_skipped_and_the_rest_imported() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    // Refuses the "Ideas" document (d2) as malformed, stores the rest.
    let engine = bind_failing(&config, |item| {
        matches!(item, tinymemory_api::StoreItem::Document { title: Some(title), .. } if title == "Ideas")
            .then(|| tinymemory_api::Error::InvalidRequest("item too large".into()))
    });

    start(&config, true).await.unwrap();
    let done = wait_until_settled(&config).await;
    assert_eq!(done.phase, ImportPhase::Done, "{done:?}");
    assert_eq!(done.imported, 4);
    let items = stored(&engine, MetaFilter::default()).await;
    assert_eq!(items.len(), 4);
    assert!(!items.iter().any(|item| item.text.contains("oolong")));
}

#[tokio::test]
async fn an_import_the_app_quit_during_resumes_on_its_own() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    let engine = bind_reference(&config);
    // The app quit while the import was running, after d1 was stored.
    write_file(
        &config.workspace_dir,
        &ImportFile {
            paused_for_credits: false,
            state: ImportState {
                phase: ImportPhase::Running,
                imported: 1,
                total: 5,
                error: None,
            },
            checkpoint: Checkpoint {
                documents: Some("d1".into()),
                ..Checkpoint::default()
            },
        },
    );

    assert!(resume_interrupted(&config).await);
    let done = wait_until_settled(&config).await;
    assert_eq!(done.phase, ImportPhase::Done, "{done:?}");
    assert_eq!(done.imported, 5);
    assert_eq!(stored(&engine, MetaFilter::default()).await.len(), 4);
}

#[tokio::test]
async fn a_stopped_or_finished_import_is_not_resumed_on_its_own() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    let engine = bind_reference(&config);
    assert!(!resume_interrupted(&config).await, "never started");

    for phase in [ImportPhase::Error, ImportPhase::Done] {
        write_file(
            &config.workspace_dir,
            &ImportFile {
                paused_for_credits: false,
                state: ImportState {
                    phase,
                    imported: 0,
                    total: 5,
                    error: None,
                },
                checkpoint: Checkpoint::default(),
            },
        );
        assert!(!resume_interrupted(&config).await, "{phase:?}");
    }
    assert!(stored(&engine, MetaFilter::default()).await.is_empty());
}

/// A persisted `Running` import with no live run: what the app leaves behind
/// when it quits mid-import after storing d1.
fn quit_mid_import(config: &Config) {
    write_file(
        &config.workspace_dir,
        &ImportFile {
            paused_for_credits: false,
            state: ImportState {
                phase: ImportPhase::Running,
                imported: 1,
                total: 5,
                error: None,
            },
            checkpoint: Checkpoint {
                documents: Some("d1".into()),
                ..Checkpoint::default()
            },
        },
    );
}

#[tokio::test]
async fn the_background_job_resumes_an_interrupted_import() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    let engine = bind_reference(&config);
    quit_mid_import(&config);

    crate::memory::bus::run_system_job(&config, crate::memory::lifecycle::jobs::BACKGROUND_JOB)
        .await;
    let done = wait_until_settled(&config).await;
    assert_eq!(done.phase, ImportPhase::Done, "{done:?}");
    assert_eq!(stored(&engine, MetaFilter::default()).await.len(), 4);
}

#[tokio::test]
async fn nothing_resumes_while_background_work_is_paused() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    let engine = bind_reference(&config);
    quit_mid_import(&config);

    assert!(!resume_interrupted_with(&config, always(true), billing(false)).await);
    assert!(stored(&engine, MetaFilter::default()).await.is_empty());
    assert_eq!(
        read_file(&config.workspace_dir).state.phase,
        ImportPhase::Running,
        "left resumable for a later, unpaused tick"
    );

    assert!(resume_interrupted_with(&config, always(false), billing(false)).await);
    assert_eq!(wait_until_settled(&config).await.phase, ImportPhase::Done);
}

/// A pause check that always answers `paused`.
fn billing(allowed: bool) -> BillingCheck {
    Arc::new(move |_: &Config| allowed)
}

fn always(paused: bool) -> PauseCheck {
    Arc::new(move || paused)
}

/// Waits until no import run is live for `config`'s workspace.
async fn wait_until_no_live_run(config: &Config) {
    for _ in 0..400 {
        let live = RUNNING.lock().unwrap().contains(&config.workspace_dir);
        if !live {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    panic!("the import run never ended");
}

#[tokio::test]
async fn a_pause_that_lands_after_the_check_stops_the_run_at_the_next_batch() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    let engine = bind_reference(&config);
    quit_mid_import(&config);

    // Not paused when the resume checks, paused by the time the run asks.
    let asked = Arc::new(AtomicUsize::new(0));
    let counter = asked.clone();
    let paused: PauseCheck = Arc::new(move || counter.fetch_add(1, Ordering::SeqCst) > 0);

    assert!(resume_interrupted_with(&config, paused, billing(false)).await);
    wait_until_no_live_run(&config).await;
    assert!(asked.load(Ordering::SeqCst) >= 2, "the run asked again");
    assert!(
        stored(&engine, MetaFilter::default()).await.is_empty(),
        "nothing uploaded once paused"
    );
    let file = read_file(&config.workspace_dir);
    assert_eq!(file.state.phase, ImportPhase::Running, "left resumable");
    assert_eq!(file.checkpoint.documents.as_deref(), Some("d1"));

    // Unpaused, the next tick finishes it from the checkpoint.
    assert!(resume_interrupted_with(&config, always(false), billing(false)).await);
    assert_eq!(wait_until_settled(&config).await.phase, ImportPhase::Done);
    assert_eq!(stored(&engine, MetaFilter::default()).await.len(), 4);
}

#[tokio::test]
async fn an_automatic_resume_that_cannot_start_is_stopped_not_retried() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    // No engine bound: memory is off, so `start` fails before any run.
    quit_mid_import(&config);

    assert!(!resume_interrupted_with(&config, always(false), billing(false)).await);
    let state = read_file(&config.workspace_dir).state;
    assert_eq!(state.phase, ImportPhase::Error);
    assert_eq!(state.imported, 1, "progress is kept");
    assert!(
        state.error.as_deref().unwrap().contains("could not resume"),
        "{state:?}"
    );
    // The next tick does not try again; the user resumes it.
    assert!(!resume_interrupted_with(&config, always(false), billing(false)).await);
    assert_eq!(
        read_file(&config.workspace_dir)
            .checkpoint
            .documents
            .as_deref(),
        Some("d1"),
        "the checkpoint read back from disk survives the failure"
    );

    // And the user's Resume continues from that checkpoint, not the start.
    let engine = bind_reference(&config);
    start(&config, true).await.unwrap();
    let done = wait_until_settled(&config).await;
    assert_eq!(done.phase, ImportPhase::Done, "{done:?}");
    assert_eq!(done.imported, 5);
    assert_eq!(
        stored(&engine, MetaFilter::default()).await.len(),
        4,
        "d1 is not re-sent"
    );
}

#[tokio::test]
async fn a_resumed_import_keeps_its_total_instead_of_rescanning() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    bind_reference(&config);
    // A total no scan of this store would produce: a resume must keep it.
    write_file(
        &config.workspace_dir,
        &ImportFile {
            paused_for_credits: false,
            state: ImportState {
                phase: ImportPhase::Error,
                imported: 1,
                total: 99,
                error: Some("unavailable".into()),
            },
            checkpoint: Checkpoint {
                documents: Some("d1".into()),
                ..Checkpoint::default()
            },
        },
    );
    let started = start(&config, true).await.unwrap();
    assert_eq!(started.total, 99);
    let done = wait_until_settled(&config).await;
    assert_eq!(
        (done.phase, done.total),
        (ImportPhase::Done, 99),
        "{done:?}"
    );
}

#[test]
fn the_import_state_is_written_whole_and_leaves_no_staging_file() {
    let tmp = tempfile::tempdir().unwrap();
    let file = ImportFile {
        paused_for_credits: false,
        state: ImportState {
            phase: ImportPhase::Running,
            imported: 3,
            total: 7,
            error: None,
        },
        checkpoint: Checkpoint {
            documents: Some("d9".into()),
            ..Checkpoint::default()
        },
    };
    write_file(tmp.path(), &file);
    write_file(tmp.path(), &file);
    let read = read_file(tmp.path());
    assert_eq!((read.state, read.checkpoint), (file.state, file.checkpoint));
    let names: Vec<String> = std::fs::read_dir(tmp.path().join("memory"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["import_state.json"]);
}

/// Calls a flaky engine has refused so far.
static FLAKY_CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[tokio::test]
async fn a_transient_failure_is_retried_within_the_run() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    FLAKY_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
    // Unavailable twice (the indexer behind: HTTP 408), then fine.
    let engine = bind_failing(&config, |_| {
        (FLAKY_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 2)
            .then(|| tinymemory_api::Error::Unavailable("WAIT_TIMEOUT".into()))
    });

    start(&config, true).await.unwrap();
    let done = wait_until_settled(&config).await;
    assert_eq!(done.phase, ImportPhase::Done, "{done:?}");
    assert_eq!(stored(&engine, MetaFilter::default()).await.len(), 5);
}

#[tokio::test]
async fn an_engine_that_stays_unavailable_is_resumed_by_the_background_job() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    bind_failing(&config, |_| {
        Some(tinymemory_api::Error::Unavailable(
            "connection refused".into(),
        ))
    });

    start(&config, true).await.unwrap();
    let stopped = wait_until_settled(&config).await;
    assert_eq!(stopped.phase, ImportPhase::Error, "{stopped:?}");
    assert!(
        stopped
            .error
            .as_deref()
            .unwrap()
            .contains("resumes on its own"),
        "{stopped:?}"
    );
    assert_eq!(
        read_file(&config.workspace_dir).state.phase,
        ImportPhase::Running,
        "left for the background job, not stopped for the user"
    );

    let engine = bind_reference(&config);
    assert!(resume_interrupted_with(&config, always(false), billing(false)).await);
    let done = wait_until_settled(&config).await;
    assert_eq!(done.phase, ImportPhase::Done, "{done:?}");
    assert_eq!(stored(&engine, MetaFilter::default()).await.len(), 5);
}

#[tokio::test]
async fn an_import_out_of_credits_resumes_only_once_automatic_runs_are_allowed() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    bind_failing(&config, out_of_credits);
    start(&config, true).await.unwrap();
    let stopped = wait_until_settled(&config).await;
    assert_eq!(stopped.phase, ImportPhase::Error, "{stopped:?}");
    assert!(read_file(&config.workspace_dir).paused_for_credits);

    // Credits still out (no free period): left paused.
    let engine = bind_reference(&config);
    assert!(!resume_interrupted_with(&config, always(false), billing(false)).await);
    assert_eq!(status(&config).phase, ImportPhase::Error);

    // Automatic runs allowed again: the background job resumes it.
    assert!(resume_interrupted_with(&config, always(false), billing(true)).await);
    let done = wait_until_settled(&config).await;
    assert_eq!(done.phase, ImportPhase::Done, "{done:?}");
    assert_eq!(stored(&engine, MetaFilter::default()).await.len(), 5);
    assert!(!read_file(&config.workspace_dir).paused_for_credits);
}

#[tokio::test]
async fn a_stop_that_is_not_about_credits_is_not_resumed_by_billing() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    bind_failing(&config, |_| {
        Some(tinymemory_api::Error::Unauthorized("sign in".into()))
    });
    start(&config, true).await.unwrap();
    assert_eq!(wait_until_settled(&config).await.phase, ImportPhase::Error);
    assert!(!read_file(&config.workspace_dir).paused_for_credits);
    assert!(!resume_interrupted_with(&config, always(false), billing(true)).await);
}

#[test]
fn only_a_self_hosted_engine_runs_automatically_until_the_free_period_check_lands() {
    let tmp = tempfile::tempdir().unwrap();
    let mut config = config_in(&tmp);
    config.memory.engine = crate::memory::engine::CORTEXDB_ENGINE.to_string();
    assert!(automatic_run_allowed(&config));
    config.memory.engine = crate::memory::engine::TINYHUMANS_ENGINE.to_string();
    assert!(
        !automatic_run_allowed(&config),
        "unknown free period is not free"
    );
}
