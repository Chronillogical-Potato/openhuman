//! The host's write guard around every bound engine.
//!
//! Every write leaves the process through [`ScrubbingEngine`]: each text a
//! `StoreItem` carries is scrubbed of secrets and PII under the host policy
//! (`security::scrub::host_policy`) before the engine sees it. Wrapping the
//! engine, rather than scrubbing at each call site, covers the paths that
//! write through TinyMemory's own types (`AgentMemory::pre_turn` /
//! `post_turn`, `Brain::ingest`, background ingests) as well as the host's.
//! Reads pass through untouched.

use std::sync::Arc;

use async_trait::async_trait;
use tinymemory_api::{
    BeliefsRequest, ConsolidateReceipt, ConsolidateRequest, EngineDescriptor, EngineHealth,
    ExplorePage, ExploreRequest, FetchPage, FetchRequest, ForgetReport, ForgetTarget, GetRequest,
    Hit, ListPage, ListRequest, MemoryEngine, RecallAnswer, RecallRequest, Result, StoreItem,
    StoreReceipt, WriteOptions,
};

/// `inner` with every stored item scrubbed first.
pub struct ScrubbingEngine {
    inner: Arc<dyn MemoryEngine>,
}

impl ScrubbingEngine {
    /// Guards `inner`.
    #[must_use]
    pub fn wrap(inner: Arc<dyn MemoryEngine>) -> Arc<dyn MemoryEngine> {
        Arc::new(Self { inner })
    }
}

/// `item` scrubbed under the host policy.
#[must_use]
pub fn scrub(item: StoreItem) -> StoreItem {
    let kind = item.kind();
    let scrubbed = tinymemory_integrations::safety::scrub_item_with(
        item,
        crate::security::scrub::host_policy(),
    );
    if scrubbed.report.changed() {
        tracing::debug!(
            kind = kind.as_str(),
            "[memory:guard] item scrubbed before store"
        );
    }
    scrubbed.value
}

#[async_trait]
impl MemoryEngine for ScrubbingEngine {
    fn descriptor(&self) -> &EngineDescriptor {
        self.inner.descriptor()
    }

    async fn health(&self) -> EngineHealth {
        self.inner.health().await
    }

    async fn recall(&self, req: RecallRequest) -> Result<RecallAnswer> {
        self.inner.recall(req).await
    }

    async fn fetch(&self, req: FetchRequest) -> Result<FetchPage> {
        self.inner.fetch(req).await
    }

    async fn store(&self, item: StoreItem) -> Result<StoreReceipt> {
        self.inner.store(scrub(item)).await
    }

    async fn store_with(&self, item: StoreItem, options: WriteOptions) -> Result<StoreReceipt> {
        self.inner.store_with(scrub(item), options).await
    }

    async fn store_many(&self, items: Vec<StoreItem>) -> Result<Vec<StoreReceipt>> {
        self.inner
            .store_many(items.into_iter().map(scrub).collect())
            .await
    }

    async fn forget(&self, target: ForgetTarget) -> Result<ForgetReport> {
        self.inner.forget(target).await
    }

    async fn list(&self, req: ListRequest) -> Result<ListPage> {
        self.inner.list(req).await
    }

    async fn explore(&self, req: ExploreRequest) -> Result<ExplorePage> {
        self.inner.explore(req).await
    }

    async fn get(&self, req: GetRequest) -> Result<Vec<Hit>> {
        self.inner.get(req).await
    }

    async fn consolidate(&self, req: ConsolidateRequest) -> Result<ConsolidateReceipt> {
        self.inner.consolidate(req).await
    }

    async fn beliefs(&self, req: BeliefsRequest) -> Result<Vec<Hit>> {
        self.inner.beliefs(req).await
    }
}

#[cfg(test)]
#[path = "guard_tests.rs"]
mod tests;
