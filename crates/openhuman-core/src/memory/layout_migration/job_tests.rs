use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use tinymemory_api::conformance::ReferenceEngine;
use tinymemory_api::{
    LearningKind, ListRequest, MemoryEngine, MemoryMeta, MetaFilter, Namespace, StoreItem,
};
use tinymemory_tools::MemoryLayout;

use super::*;
use crate::memory::error::MemoryError;
use crate::memory::layout_migration::copy::Engines;
use crate::memory::layout_migration::map::{FlowPlacement, Placement};

/// Two in-memory engines and the switches a real host keeps.
struct FakeHost {
    legacy: Arc<ReferenceEngine>,
    tree: Arc<ReferenceEngine>,
    switched: AtomicBool,
    free: AtomicBool,
    shared: bool,
    /// Asks for free_now: after this many, moving stops being free.
    free_for: AtomicUsize,
    /// Written to the legacy tree as the switch happens (a turn racing it).
    racing_write: Mutex<Option<StoreItem>>,
}

impl FakeHost {
    async fn with(learnings: usize) -> Self {
        let legacy = Arc::new(ReferenceEngine::new());
        for i in 0..learnings {
            legacy.store(fact(&format!("fact {i}"))).await.unwrap();
        }
        Self {
            legacy,
            tree: Arc::new(ReferenceEngine::new()),
            switched: AtomicBool::new(false),
            free: AtomicBool::new(true),
            shared: false,
            free_for: AtomicUsize::new(usize::MAX),
            racing_write: Mutex::new(None),
        }
    }
}

fn fact(text: &str) -> StoreItem {
    StoreItem::learning(text, LearningKind::Fact, 0.5, MemoryMeta::default())
}

async fn count(engine: &dyn MemoryEngine) -> usize {
    let mut total = 0;
    let mut cursor = None;
    loop {
        let mut request = ListRequest::new(MetaFilter::default(), 100);
        request.cursor = cursor;
        let page = engine.list(request).await.unwrap();
        total += page.items.len();
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => return total,
        }
    }
}

#[async_trait::async_trait]
impl LayoutHost for FakeHost {
    fn engines(&self, _: &Config) -> MemoryResult<Engines> {
        Ok(Engines {
            legacy: self.legacy.clone(),
            tree: self.tree.clone(),
        })
    }

    fn placement(&self, _: &Config) -> MemoryResult<Placement> {
        Ok(Placement {
            layout: MemoryLayout::new(Namespace::ROOT)
                .map_err(|e| MemoryError::invalid(e.to_string()))?,
            chat_node: "ws:main".parse().unwrap(),
            flows: FlowPlacement::WithRoot,
        })
    }

    fn is_switched(&self, _: &Config) -> bool {
        self.switched.load(Ordering::SeqCst)
    }

    fn switch(&self, _: &Config) -> MemoryResult<()> {
        if let Some(item) = self.racing_write.lock().unwrap().take() {
            // Synchronous on purpose: the reference engine stores in memory.
            futures::executor::block_on(self.legacy.store(item)).unwrap();
        }
        self.switched.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn free_now(&self, _: &Config) -> bool {
        let left = self.free_for.fetch_sub(1, Ordering::SeqCst);
        self.free.load(Ordering::SeqCst) && left > 0
    }

    fn shared_legacy(&self, _: &Config) -> bool {
        self.shared
    }
}

async fn go(config: &Config, host: &FakeHost, trigger: Trigger) -> Outcome {
    run(config, host, trigger, || async { false })
        .await
        .unwrap()
}

#[tokio::test]
async fn no_legacy_memory_switches_at_once_and_nothing_else() {
    let tmp = tempfile::tempdir().unwrap();
    let config = crate::memory::test_fixtures::config_in(&tmp);
    let host = FakeHost::with(0).await;
    assert_eq!(
        go(&config, &host, Trigger::Auto).await,
        Outcome::NothingToMove
    );
    assert!(host.is_switched(&config));
    assert_eq!(
        go(&config, &host, Trigger::Auto).await,
        Outcome::Done,
        "settled"
    );
}

#[tokio::test]
async fn an_automatic_run_waits_while_moving_is_not_free() {
    let tmp = tempfile::tempdir().unwrap();
    let config = crate::memory::test_fixtures::config_in(&tmp);
    let host = FakeHost::with(3).await;
    host.free.store(false, Ordering::SeqCst);
    assert_eq!(go(&config, &host, Trigger::Auto).await, Outcome::NotFree);
    assert!(!host.is_switched(&config));
    assert_eq!(count(host.tree.as_ref()).await, 0);
    assert_eq!(
        go(&config, &host, Trigger::Manual { takeover: false }).await,
        Outcome::Done,
        "the user's own start does not wait for a free period"
    );
}

#[tokio::test]
async fn a_shared_legacy_tree_moves_only_with_consent() {
    let tmp = tempfile::tempdir().unwrap();
    let config = crate::memory::test_fixtures::config_in(&tmp);
    let mut host = FakeHost::with(3).await;
    host.shared = true;
    assert_eq!(
        go(&config, &host, Trigger::Auto).await,
        Outcome::NeedsTakeover
    );
    assert_eq!(
        go(&config, &host, Trigger::Manual { takeover: false }).await,
        Outcome::NeedsTakeover
    );
    assert_eq!(
        go(&config, &host, Trigger::Manual { takeover: true }).await,
        Outcome::Done
    );
    assert_eq!(
        go(&config, &host, Trigger::Auto).await,
        Outcome::Done,
        "consent is kept"
    );
}

#[tokio::test]
async fn a_free_run_moves_switches_and_cleans_up() {
    let tmp = tempfile::tempdir().unwrap();
    let config = crate::memory::test_fixtures::config_in(&tmp);
    let host = FakeHost::with(120).await;
    assert_eq!(go(&config, &host, Trigger::Auto).await, Outcome::Done);
    assert!(host.is_switched(&config));
    assert_eq!(count(host.tree.as_ref()).await, 120);
    assert_eq!(count(host.legacy.as_ref()).await, 0);
}

#[tokio::test]
async fn a_free_period_ending_mid_run_pauses_before_the_switch_and_resumes() {
    let tmp = tempfile::tempdir().unwrap();
    let config = crate::memory::test_fixtures::config_in(&tmp);
    let host = FakeHost::with(120).await;
    // Free at the start and for the first page, then not.
    host.free_for.store(2, Ordering::SeqCst);
    assert_eq!(go(&config, &host, Trigger::Auto).await, Outcome::Paused);
    assert!(
        !host.is_switched(&config),
        "reads stay on the full legacy tree"
    );
    assert!(count(host.tree.as_ref()).await < 120);

    host.free_for.store(usize::MAX, Ordering::SeqCst);
    assert_eq!(go(&config, &host, Trigger::Auto).await, Outcome::Done);
    assert_eq!(count(host.tree.as_ref()).await, 120, "no item twice");
    assert_eq!(count(host.legacy.as_ref()).await, 0);
}

#[tokio::test]
async fn a_write_racing_the_switch_is_caught_up() {
    let tmp = tempfile::tempdir().unwrap();
    let config = crate::memory::test_fixtures::config_in(&tmp);
    let host = FakeHost::with(5).await;
    *host.racing_write.lock().unwrap() = Some(fact("written as the switch happened"));
    assert_eq!(go(&config, &host, Trigger::Auto).await, Outcome::Done);
    let texts: Vec<String> = host
        .tree
        .list(ListRequest::new(MetaFilter::default(), 50))
        .await
        .unwrap()
        .items
        .into_iter()
        .map(|hit| hit.text)
        .collect();
    assert_eq!(texts.len(), 6);
    assert!(texts.iter().any(|t| t == "written as the switch happened"));
    assert_eq!(count(host.legacy.as_ref()).await, 0);
}
