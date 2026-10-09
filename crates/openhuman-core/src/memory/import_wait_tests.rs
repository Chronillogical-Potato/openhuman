//! Tests of how the import stores its batches: accepted, with one
//! best-effort wait for the last batch to be listed before it is done.

use std::sync::Arc;

use super::tests::{legacy_workspace, wait_until_settled};
use super::*;
use crate::memory::test_fixtures::config_in;

/// A reference engine that records the wait each bulk store asks for.
#[derive(Default)]
struct WaitRecordingEngine {
    inner: tinymemory_api::conformance::ReferenceEngine,
    waits: std::sync::Mutex<Vec<tinymemory_api::WaitFor>>,
    /// A visible bulk store times out, as on a listing that lags.
    never_listed: bool,
}

#[async_trait::async_trait]
impl tinymemory_api::MemoryEngine for WaitRecordingEngine {
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
        self.inner.store(item).await
    }
    async fn store_many_with(
        &self,
        items: Vec<tinymemory_api::StoreItem>,
        options: tinymemory_api::WriteOptions,
    ) -> tinymemory_api::Result<Vec<tinymemory_api::StoreReceipt>> {
        self.waits.lock().unwrap().push(options.wait);
        if self.never_listed && options.wait == tinymemory_api::WaitFor::Visible {
            return Err(tinymemory_api::Error::Unavailable(
                "accepted but did not become readable within 30s".into(),
            ));
        }
        self.inner.store_many(items).await
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

/// Batches are stored accepted (nothing reads them back mid-import); the
/// last is stored again visible, once, before the import says it is done.
#[tokio::test]
async fn batches_are_stored_accepted_and_the_last_waited_for_once() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    let engine = Arc::new(WaitRecordingEngine::default());
    crate::memory::engine::install_test_engine(&config.workspace_dir, engine.clone());

    start(&config, true).await.unwrap();
    let done = wait_until_settled(&config).await;
    assert_eq!(done.phase, ImportPhase::Done, "{done:?}");
    assert_eq!(
        done.imported, 5,
        "the final wait is a replay, not a recount"
    );
    assert_eq!(
        *engine.waits.lock().unwrap(),
        vec![
            tinymemory_api::WaitFor::Accepted,
            tinymemory_api::WaitFor::Visible
        ],
        "one accepted batch, then the visible wait for it"
    );
    assert!(
        !listed_unconfirmed(&config.workspace_dir),
        "a confirmed wait leaves whole-scope cleanup allowed"
    );
}

/// The end-of-import wait is best-effort: a listing that never catches up
/// (every visible store times out) still ends the import `Done`, instead of
/// leaving it to resume into the same wait for ever.
#[tokio::test]
async fn a_last_batch_never_listed_still_finishes_the_import() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    let engine = Arc::new(WaitRecordingEngine {
        never_listed: true,
        ..WaitRecordingEngine::default()
    });
    crate::memory::engine::install_test_engine(&config.workspace_dir, engine.clone());

    start(&config, true).await.unwrap();
    let done = wait_until_settled(&config).await;
    assert_eq!(done.phase, ImportPhase::Done, "{done:?}");
    assert_eq!(done.imported, 5);
    assert!(done.error.is_none(), "{done:?}");
    let waits = engine.waits.lock().unwrap().clone();
    assert_eq!(waits[0], tinymemory_api::WaitFor::Accepted, "{waits:?}");
    assert!(
        waits[1..]
            .iter()
            .all(|wait| *wait == tinymemory_api::WaitFor::Visible)
            && waits.len() > 2,
        "the final wait was retried, then given up: {waits:?}"
    );
    assert!(
        listed_unconfirmed(&config.workspace_dir),
        "an unconfirmed wait is recorded for the migration's cleanup"
    );
}
