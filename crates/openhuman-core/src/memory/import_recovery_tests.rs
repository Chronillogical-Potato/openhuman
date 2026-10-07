//! Tests of how the import recovers: an atomic state file, a resume that
//! keeps its total, transient failures retried and resumed, a credits pause
//! lifted only when automatic runs are allowed, and refused items kept and
//! retried.

use super::tests::{
    after_document, always, billing, bind_failing, legacy_workspace, out_of_credits,
    wait_until_no_live_run, wait_until_settled,
};
use super::*;
use crate::memory::error::INVALID_REQUEST;
use crate::memory::test_fixtures::{bind_reference, config_in, stored};
use tinymemory_api::MetaFilter;

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
            failed: Vec::new(),
            retrying: false,
            state: ImportState {
                phase: ImportPhase::Error,
                imported: 1,
                total: 99,
                error: Some("unavailable".into()),
                failed: 0,
            },
            checkpoint: after_document("d1"),
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
        failed: Vec::new(),
        retrying: false,
        state: ImportState {
            phase: ImportPhase::Running,
            imported: 3,
            total: 7,
            error: None,
            failed: 0,
        },
        checkpoint: after_document("d9"),
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

/// An engine refusal of the "Ideas" document while `$flag` is set, one flag
/// per test so tests running in parallel never toggle each other's.
macro_rules! ideas_refusal {
    ($flag:ident, $refuse:ident) => {
        static $flag: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
        fn $refuse(item: &tinymemory_api::StoreItem) -> Option<tinymemory_api::Error> {
            let ideas = matches!(item, tinymemory_api::StoreItem::Document { title: Some(title), .. } if title == "Ideas");
            (ideas && $flag.load(std::sync::atomic::Ordering::SeqCst))
                .then(|| tinymemory_api::Error::InvalidRequest("item too large".into()))
        }
    };
}

ideas_refusal!(REFUSE_IDEAS_KEPT, refuse_ideas_kept);
ideas_refusal!(REFUSE_IDEAS_STOPPED, refuse_ideas_stopped);
ideas_refusal!(REFUSE_IDEAS_QUIT, refuse_ideas_quit);

#[tokio::test]
async fn a_refused_item_is_kept_and_a_retry_stores_it() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    REFUSE_IDEAS_KEPT.store(true, std::sync::atomic::Ordering::SeqCst);
    let engine = bind_failing(&config, refuse_ideas_kept);

    start(&config, true).await.unwrap();
    let done = wait_until_settled(&config).await;
    assert_eq!(
        (done.phase, done.imported, done.failed),
        (ImportPhase::Done, 4, 1)
    );
    let file = read_file(&config.workspace_dir);
    assert_eq!(file.failed.len(), 1);
    assert_eq!(file.failed[0].id, "memory_docs:d2");
    assert!(
        file.failed[0].reason.contains("item too large"),
        "{:?}",
        file.failed
    );

    // Still refused: kept, with the reason.
    retry_failed(&config).await.unwrap();
    let again = wait_until_settled(&config).await;
    assert_eq!(
        (again.phase, again.failed),
        (ImportPhase::Done, 1),
        "{again:?}"
    );

    // The engine takes it now: the list empties and the item is stored.
    REFUSE_IDEAS_KEPT.store(false, std::sync::atomic::Ordering::SeqCst);
    retry_failed(&config).await.unwrap();
    let fixed = wait_until_settled(&config).await;
    assert_eq!(
        (fixed.phase, fixed.imported, fixed.failed),
        (ImportPhase::Done, 5, 0),
        "{fixed:?}"
    );
    assert!(read_file(&config.workspace_dir).failed.is_empty());
    let items = stored(&engine, MetaFilter::default()).await;
    assert!(items.iter().any(|item| item.text.contains("oolong")));
}

#[tokio::test]
async fn retrying_needs_a_finished_import_with_failed_items() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    bind_reference(&config);
    let error = retry_failed(&config).await.unwrap_err();
    assert_eq!(error.code(), INVALID_REQUEST);
    start(&config, true).await.unwrap();
    assert_eq!(wait_until_settled(&config).await.failed, 0);
    assert_eq!(
        retry_failed(&config).await.unwrap_err().code(),
        INVALID_REQUEST
    );
}

#[tokio::test]
async fn a_retry_stopped_by_the_engine_stays_retryable() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    REFUSE_IDEAS_STOPPED.store(true, std::sync::atomic::Ordering::SeqCst);
    bind_failing(&config, refuse_ideas_stopped);
    start(&config, true).await.unwrap();
    assert_eq!(wait_until_settled(&config).await.failed, 1);

    // The engine is down for the retry: it stops, still finished, still listed.
    bind_failing(&config, |_| {
        Some(tinymemory_api::Error::Unauthorized("sign in".into()))
    });
    retry_failed(&config).await.unwrap();
    let stopped = wait_until_settled(&config).await;
    assert_eq!(
        (stopped.phase, stopped.failed),
        (ImportPhase::Done, 1),
        "{stopped:?}"
    );
    // A retry is the user's action: it says to press Retry, not that it
    // resumes on its own.
    let reason = stopped.error.as_deref().unwrap();
    assert!(reason.contains("press Retry again"), "{reason}");
    assert!(!reason.contains("on its own"), "{reason}");

    // Back up: the same retry goes through.
    REFUSE_IDEAS_STOPPED.store(false, std::sync::atomic::Ordering::SeqCst);
    let engine = bind_failing(&config, refuse_ideas_stopped);
    retry_failed(&config).await.unwrap();
    let done = wait_until_settled(&config).await;
    assert_eq!(
        (done.phase, done.failed, done.error),
        (ImportPhase::Done, 0, None)
    );
    assert!(stored(&engine, MetaFilter::default())
        .await
        .iter()
        .any(|i| i.text.contains("oolong")));
}

#[tokio::test]
async fn a_credits_pause_whose_resume_fails_is_not_retried_every_tick() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    bind_failing(&config, out_of_credits);
    start(&config, true).await.unwrap();
    wait_until_settled(&config).await;
    assert!(read_file(&config.workspace_dir).paused_for_credits);

    // The legacy store is gone: the automatic resume cannot start.
    std::fs::remove_file(config.workspace_dir.join("memory").join("memory.db")).unwrap();
    bind_reference(&config);
    assert!(!resume_interrupted_with(&config, always(false), billing(true)).await);
    assert!(!read_file(&config.workspace_dir).paused_for_credits);
    assert!(!resume_interrupted_with(&config, always(false), billing(true)).await);
}

#[tokio::test]
async fn a_retry_the_app_quit_during_resumes_as_a_retry() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    REFUSE_IDEAS_QUIT.store(true, std::sync::atomic::Ordering::SeqCst);
    let engine = bind_failing(&config, refuse_ideas_quit);
    start(&config, true).await.unwrap();
    assert_eq!(wait_until_settled(&config).await.failed, 1);

    // The app quit mid-retry: the state file says a retry is Running.
    let mut file = read_file(&config.workspace_dir);
    file.state.phase = ImportPhase::Running;
    file.retrying = true;
    write_file(&config.workspace_dir, &file);

    // The background resume carries on with the retry, not the import, and
    // the engine takes the item now.
    REFUSE_IDEAS_QUIT.store(false, std::sync::atomic::Ordering::SeqCst);
    assert!(resume_interrupted_with(&config, always(false), billing(false)).await);
    let done = wait_until_settled(&config).await;
    assert_eq!(
        (done.phase, done.failed),
        (ImportPhase::Done, 0),
        "{done:?}"
    );
    assert!(!read_file(&config.workspace_dir).retrying);
    assert!(stored(&engine, MetaFilter::default())
        .await
        .iter()
        .any(|item| item.text.contains("oolong")));
}

/// A finished import with `d2` ("Ideas") still to retry, and the state of a
/// retry the app quit during.
fn quit_mid_retry(config: &Config) {
    let mut file = ImportFile::default();
    file.state = ImportState {
        phase: ImportPhase::Running,
        imported: 4,
        total: 5,
        error: None,
        failed: 1,
    };
    file.failed = vec![FailedItem {
        id: "memory_docs:d2".into(),
        reason: "item too large".into(),
    }];
    file.retrying = true;
    write_file(&config.workspace_dir, &file);
}

#[tokio::test]
async fn a_retry_that_cannot_resume_stays_finished_and_retryable() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    quit_mid_retry(&config);

    // No engine is bound: the background resume of the retry cannot start.
    assert!(!resume_interrupted_with(&config, always(false), billing(false)).await);
    let file = read_file(&config.workspace_dir);
    assert_eq!(
        (file.state.phase, file.failed.len()),
        (ImportPhase::Done, 1)
    );
    assert!(!file.retrying);
    assert!(file
        .state
        .error
        .as_deref()
        .unwrap()
        .contains("press Retry again"));

    // Once an engine is there, Retry goes through.
    let engine = bind_reference(&config);
    retry_failed(&config).await.unwrap();
    let done = wait_until_settled(&config).await;
    assert_eq!(
        (done.phase, done.failed),
        (ImportPhase::Done, 0),
        "{done:?}"
    );
    assert!(stored(&engine, MetaFilter::default())
        .await
        .iter()
        .any(|item| item.text.contains("oolong")));
}

#[tokio::test]
async fn a_resumed_retry_stops_when_background_work_is_paused() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    let engine = bind_reference(&config);
    quit_mid_retry(&config);

    // Paused after the resume check: the retry stores nothing and is left
    // as a running retry for the next unpaused tick.
    let checks = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let seen = checks.clone();
    let paused: PauseCheck =
        std::sync::Arc::new(move || seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst) > 0);
    assert!(resume_interrupted_with(&config, paused, billing(false)).await);
    wait_until_no_live_run(&config).await;
    let file = read_file(&config.workspace_dir);
    assert_eq!(file.state.phase, ImportPhase::Running);
    assert!(file.retrying);
    assert_eq!(file.failed.len(), 1);
    assert!(stored(&engine, MetaFilter::default()).await.is_empty());
}

#[tokio::test]
async fn retry_is_accepted_for_a_retry_the_app_quit_during() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    let engine = bind_reference(&config);
    quit_mid_retry(&config);

    // The user presses Retry before the background job got to it.
    retry_failed(&config).await.unwrap();
    let done = wait_until_settled(&config).await;
    assert_eq!(
        (done.phase, done.failed),
        (ImportPhase::Done, 0),
        "{done:?}"
    );
    assert!(stored(&engine, MetaFilter::default())
        .await
        .iter()
        .any(|item| item.text.contains("oolong")));
}
