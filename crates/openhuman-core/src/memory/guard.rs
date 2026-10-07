//! The host's write guard around every bound engine.
//!
//! Every write leaves the process through [`ScrubbingEngine`]: each text a
//! `StoreItem` carries is scrubbed of secrets and PII under the host policy
//! (`security::scrub::host_policy`) before the engine sees it. Wrapping the
//! engine, rather than scrubbing at each call site, covers the paths that
//! write through TinyMemory's own types (`AgentMemory::pre_turn` /
//! `post_turn`, `Brain::ingest`, background ingests) as well as the host's.
//! Reads pass through untouched.
//!
//! The same wrapper times every engine call: one `[memory:timing]` debug
//! line with the op, engine, `elapsed_ms`, outcome (`ok` or the error code)
//! and a result count, never user content (tinyhumansai/backend#1405).

use std::future::Future;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use tinymemory_api::{
    BeliefsRequest, ConsolidateReceipt, ConsolidateRequest, EngineDescriptor, EngineHealth,
    EraseReport, EraseRequest, ExplorePage, ExploreRequest, ExportPage, FetchPage, FetchRequest,
    ForgetReport, ForgetTarget, GetRequest, Hit, ListPage, ListRequest, MemoryEngine, RecallAnswer,
    RecallRequest, Result, StoreItem, StoreReceipt, WaitFor, WriteOptions,
};

use super::error::MemoryError;

/// `inner` with every stored item scrubbed first, and every call timed.
pub struct ScrubbingEngine {
    inner: Arc<dyn MemoryEngine>,
    id: String,
}

impl ScrubbingEngine {
    /// Guards `inner`.
    #[must_use]
    pub fn wrap(inner: Arc<dyn MemoryEngine>) -> Arc<dyn MemoryEngine> {
        let id = inner.descriptor().id.to_string();
        Arc::new(Self { inner, id })
    }

    /// Runs `call` and logs its timing; `count` sizes a success.
    async fn timed<T>(
        &self,
        op: &'static str,
        call: impl Future<Output = Result<T>>,
        count: impl FnOnce(&T) -> usize,
    ) -> Result<T> {
        let started = Instant::now();
        let result = call.await;
        let elapsed_ms = started.elapsed().as_millis() as u64;
        let outcome = outcome(&result);
        let count = result.as_ref().map_or(0, count);
        tracing::debug!(
            op,
            engine = %self.id,
            elapsed_ms,
            outcome,
            count,
            "[memory:timing]"
        );
        result
    }
}

/// `ok`, or the stable code of the error a call failed with.
fn outcome<T>(result: &Result<T>) -> &'static str {
    match result {
        Ok(_) => "ok",
        Err(error) => MemoryError::from(error.clone()).code(),
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
        let started = Instant::now();
        let health = self.inner.health().await;
        let outcome = match &health {
            EngineHealth::Ok => "ok",
            EngineHealth::Degraded(_) => "degraded",
            EngineHealth::Down(_) => "down",
        };
        tracing::debug!(
            op = "health",
            engine = %self.id,
            elapsed_ms = started.elapsed().as_millis() as u64,
            outcome,
            "[memory:timing]"
        );
        health
    }

    async fn recall(&self, req: RecallRequest) -> Result<RecallAnswer> {
        self.timed("recall", self.inner.recall(req), |answer| {
            answer.citations.len()
        })
        .await
    }

    async fn fetch(&self, req: FetchRequest) -> Result<FetchPage> {
        self.timed("fetch", self.inner.fetch(req), |page| page.hits.len())
            .await
    }

    async fn store(&self, item: StoreItem) -> Result<StoreReceipt> {
        self.timed("store", self.inner.store(scrub(item)), |_| 1)
            .await
    }

    async fn store_with(&self, item: StoreItem, options: WriteOptions) -> Result<StoreReceipt> {
        let op = match options.wait {
            WaitFor::Accepted => "store_accepted",
            WaitFor::Visible => "store",
        };
        self.timed(op, self.inner.store_with(scrub(item), options), |_| 1)
            .await
    }

    async fn store_many(&self, items: Vec<StoreItem>) -> Result<Vec<StoreReceipt>> {
        let items = items.into_iter().map(scrub).collect();
        self.timed("store_many", self.inner.store_many(items), Vec::len)
            .await
    }

    async fn forget(&self, target: ForgetTarget) -> Result<ForgetReport> {
        self.timed("forget", self.inner.forget(target), |report| {
            report.forgotten
        })
        .await
    }

    async fn list(&self, req: ListRequest) -> Result<ListPage> {
        self.timed("list", self.inner.list(req), |page| page.items.len())
            .await
    }

    async fn export(&self, req: ListRequest) -> Result<ExportPage> {
        self.timed("export", self.inner.export(req), |page| page.items.len())
            .await
    }

    async fn erase(&self, req: EraseRequest) -> Result<EraseReport> {
        self.timed("erase", self.inner.erase(req), |report| {
            report.erased_scopes
        })
        .await
    }

    async fn explore(&self, req: ExploreRequest) -> Result<ExplorePage> {
        self.timed("explore", self.inner.explore(req), |page| {
            page.buckets.len()
        })
        .await
    }

    async fn get(&self, req: GetRequest) -> Result<Vec<Hit>> {
        self.timed("get", self.inner.get(req), Vec::len).await
    }

    async fn consolidate(&self, req: ConsolidateRequest) -> Result<ConsolidateReceipt> {
        self.timed("consolidate", self.inner.consolidate(req), |receipt| {
            receipt.jobs.len()
        })
        .await
    }

    async fn beliefs(&self, req: BeliefsRequest) -> Result<Vec<Hit>> {
        self.timed("beliefs", self.inner.beliefs(req), Vec::len)
            .await
    }
}

#[cfg(test)]
#[path = "guard_tests.rs"]
mod tests;
