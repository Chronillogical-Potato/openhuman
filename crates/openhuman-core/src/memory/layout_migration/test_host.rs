//! A host over two in-memory engines, for the migration's tests.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use tinymemory_api::conformance::ReferenceEngine;
use tinymemory_api::{
    LearningKind, ListRequest, MemoryEngine, MemoryMeta, MetaFilter, Namespace, StoreItem,
};
use tinymemory_tools::MemoryLayout;

use super::claim::ClaimKey;
use super::copy::Engines;
use super::host::LayoutHost;
use super::map::{FlowPlacement, Placement};
use crate::config::Config;
use crate::memory::error::{MemoryError, MemoryResult};

/// Two in-memory engines and the switches a real host keeps.
pub(crate) struct FakeHost {
    pub(crate) legacy: Arc<ReferenceEngine>,
    pub(crate) tree: Arc<ReferenceEngine>,
    pub(crate) switched: AtomicBool,
    pub(crate) free: AtomicBool,
    /// The claim when the legacy tree is shared.
    pub(crate) claim: Option<ClaimKey>,
    /// Asks for free_now: after this many, moving stops being free.
    pub(crate) free_for: AtomicUsize,
    /// Written to the legacy tree as the switch happens (a turn racing it).
    pub(crate) racing_write: Mutex<Option<StoreItem>>,
}

impl FakeHost {
    pub(crate) async fn with(learnings: usize) -> Self {
        let legacy = Arc::new(ReferenceEngine::new());
        for i in 0..learnings {
            legacy.store(fact(&format!("fact {i}"))).await.unwrap();
        }
        Self {
            legacy,
            tree: Arc::new(ReferenceEngine::new()),
            switched: AtomicBool::new(false),
            free: AtomicBool::new(true),
            claim: None,
            free_for: AtomicUsize::new(usize::MAX),
            racing_write: Mutex::new(None),
        }
    }
}

pub(crate) fn fact(text: &str) -> StoreItem {
    StoreItem::learning(text, LearningKind::Fact, 0.5, MemoryMeta::default())
}

pub(crate) async fn count(engine: &dyn MemoryEngine) -> usize {
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

    async fn switch(&self, _: &Config) -> MemoryResult<()> {
        let racing = self.racing_write.lock().unwrap().take();
        if let Some(item) = racing {
            self.legacy.store(item).await.unwrap();
        }
        self.switched.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn free_now(&self, _: &Config) -> bool {
        let left = self.free_for.fetch_sub(1, Ordering::SeqCst);
        self.free.load(Ordering::SeqCst) && left > 0
    }

    fn legacy_claim(&self, _: &Config) -> MemoryResult<Option<ClaimKey>> {
        Ok(self.claim.clone())
    }
}
