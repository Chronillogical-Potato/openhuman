//! JSON-RPC handlers for the agent-experience store, and the adapter that lets
//! it read and write through the **bound memory driver** instead of an
//! in-process engine handle (openhuman#5560).

use serde::{Deserialize, Serialize};

use crate::agent::experience::store::{
    retrieve_across_stores, AgentExperienceStore, ExperienceQuery,
};
use crate::agent::experience::types::{AgentExperience, ExperienceHit};
use crate::config::Config;
use crate::memory::api::health::MemoryHealth;
// `MemoryCore` and `MemoryRecall` are deliberately NOT imported: their methods
// are reached on a `dyn MemoryProvider` receiver, where supertrait methods are
// inherent object candidates rather than in-scope-trait candidates — so an
// import of either would be flagged unused and fail `clippy -D warnings`.
use crate::memory::api::provider::MemoryProvider;
use crate::memory::api::recall::OwnedRecallOpts;
use crate::memory::api::types::{
    MemoryCategory, MemoryEntry, MemoryTaint, NamespaceSummary, RecallOpts,
};
use crate::memory::Memory;
use crate::rpc::RpcOutcome;
use async_trait::async_trait;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// The [`Memory`] view of a bound memory driver.
///
/// # Why this exists
///
/// [`AgentExperienceStore`] is built on `Arc<dyn Memory>` — the storage trait
/// `tinymemory-api` exports — and the driver contract has no door that hands one
/// out. Before #5560 the host closed that gap with the in-process engine's own
/// `MemoryClient::memory_handle()`, which meant every experience read and write
/// went through a **second** engine over the same SQLite file as the loaded
/// TinyMemory module. This adapter closes it the other way round: `Memory`'s
/// methods are a subset of what `MemoryCore` and `MemoryRecall` already
/// promise, so the whole trait can be served from the bound driver with nothing
/// added to the contract.
///
/// # It wraps the driver, not the guard, and that is deliberate
///
/// [`MemoryGuard`](crate::memory::guard::MemoryGuard) truncates
/// `store` content at `capture_max_chars`, which
/// `MemoryHooksConfig::default()` sets to **500**. An `AgentExperience` is
/// stored as base64 of its serialized JSON — precisely so the free-text
/// scrubber cannot rewrite a Luhn-valid millisecond timestamp and corrupt the
/// payload (#5209) — and truncated base64 does not decode, so the record would
/// silently vanish on read. That is the same class of bug #5209 fixed, so the
/// pre-#5560 behaviour is preserved exactly: no policy layer between this store
/// and the driver. The store runs the full scrubber over its own free-text
/// fields before serialization (`store::redact_experience`), which is what
/// keeps that safe rather than merely unguarded.
///
/// # Home
///
/// This belongs in `crates/openhuman-core/src/memory/`, next to `binding`, not under
/// `agent::experience` — it is contract-to-trait plumbing, not an
/// agent-experience concept. It sits here because both of its callers do
/// (`open_store_in_subdir` below, and the session builder's
/// `shared_experience_memory`).
pub struct DriverMemory {
    provider: Arc<dyn MemoryProvider>,
}

impl DriverMemory {
    /// Wrap an already-resolved driver.
    pub fn new(provider: Arc<dyn MemoryProvider>) -> Self {
        Self { provider }
    }

    /// The driver bound for `config`'s workspace and shared `memory` subtree.
    ///
    /// # Errors
    ///
    /// Only binding-cache lock poisoning: an inadmissible driver falls back to
    /// the null driver loudly rather than failing here (kernel.md §3.7).
    pub fn for_config(config: &Config) -> Result<Arc<dyn Memory>, String> {
        let binding = crate::memory::binding::for_config(config)?;
        log::debug!(
            "[agent-experience] bound shared memory subtree driver='{}'",
            binding.driver_id()
        );
        let memory: Arc<dyn Memory> = Arc::new(Self::new(binding.provider().clone()));
        Ok(memory)
    }

    /// The driver bound for one memory subtree of `config`'s workspace.
    ///
    /// into dedicated memory. Each subtree is its own binding and therefore its
    /// own store, which is what makes `dedicatedMemory` isolation hold.
    ///
    /// # Errors
    ///
    /// As [`Self::for_config`].
    pub fn for_subtree(config: &Config, memory_subdir: &str) -> Result<Arc<dyn Memory>, String> {
        let binding = crate::memory::binding::for_subtree(
            &config.workspace_dir,
            memory_subdir,
            &config.subsystems.memory,
        )?;
        log::debug!(
            "[agent-experience] bound memory subtree '{memory_subdir}' driver='{}'",
            binding.driver_id()
        );
        let memory: Arc<dyn Memory> = Arc::new(Self::new(binding.provider().clone()));
        Ok(memory)
    }
}

#[async_trait]
impl Memory for DriverMemory {
    /// The **driver's** id, not a synthetic name: this string reaches logs and
    /// status output, which should name whatever actually stores the bytes.
    fn name(&self) -> &str {
        self.provider.driver_id()
    }

    async fn store(
        &self,
        namespace: &str,
        key: &str,
        content: &str,
        category: MemoryCategory,
        session_id: Option<&str>,
    ) -> anyhow::Result<()> {
        Ok(self
            .provider
            .store(
                namespace,
                key,
                content,
                category,
                session_id,
                MemoryTaint::Internal,
            )
            .await?)
    }

    /// Overridden rather than inherited. The trait's default *bails* for any
    /// taint other than `Internal`, so inheriting it would turn a sync path's
    /// `ExternalSync` write into an error against a driver that records taint
    /// perfectly well.
    async fn store_with_taint(
        &self,
        namespace: &str,
        key: &str,
        content: &str,
        category: MemoryCategory,
        session_id: Option<&str>,
        taint: MemoryTaint,
    ) -> anyhow::Result<()> {
        Ok(self
            .provider
            .store(namespace, key, content, category, session_id, taint)
            .await?)
    }

    /// `scope` is passed explicitly, never left to ambient state.
    ///
    /// The per-turn source allowlist is a `tokio::task_local` and task-locals
    /// do not cross the bus — a module reads an unset one as *unrestricted*, so
    /// a call that relied on the driver picking it up would fail open. Rendering
    /// it host-side with `source_scope::as_bus_scope()` is the rule the whole
    /// memory seam follows.
    async fn recall(
        &self,
        query: &str,
        limit: usize,
        opts: RecallOpts<'_>,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        let owned: OwnedRecallOpts = opts.into();
        let scope = crate::memory::source_scope::as_bus_scope();
        Ok(self
            .provider
            .recall(query, limit, &owned, scope.as_ref())
            .await?)
    }

    /// Situational preferences (Lane B) reach memory through **this** adapter:
    /// the session's `Arc<dyn Memory>` is a `DriverMemory` on every
    /// module-backed install. This used to sit on the trait's default — empty,
    /// "the documented opt-out for a backend that cannot answer it" — on the
    /// belief that Lane B was reached through some other handle. It was not,
    /// so the lane silently injected nothing for everyone (#6041).
    ///
    /// The contract has no vector-threshold member; the body is the guard
    /// path's, over `MemoryRetrieval::recall_namespace_scored`, filtered on the
    /// vector component. A driver without the retrieval family still answers
    /// empty; a driver that has it and fails answers `Err`, which Lane B reads
    /// as "no block" rather than as a broken turn.
    async fn recall_relevant_by_vector(
        &self,
        namespace: &str,
        query: &str,
        limit: usize,
        min_vector_similarity: f64,
    ) -> anyhow::Result<Vec<(String, String)>> {
        Ok(crate::memory::preferences::recall_by_vector_over(
            self.provider.as_ref(),
            namespace,
            query,
            limit,
            min_vector_similarity,
        )
        .await?)
    }

    async fn get(&self, namespace: &str, key: &str) -> anyhow::Result<Option<MemoryEntry>> {
        Ok(self.provider.get(namespace, key).await?)
    }

    async fn list(
        &self,
        namespace: Option<&str>,
        category: Option<&MemoryCategory>,
        session_id: Option<&str>,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        Ok(self.provider.list(namespace, category, session_id).await?)
    }

    async fn forget(&self, namespace: &str, key: &str) -> anyhow::Result<bool> {
        Ok(self.provider.forget(namespace, key).await?)
    }

    async fn namespace_summaries(&self) -> anyhow::Result<Vec<NamespaceSummary>> {
        Ok(self.provider.namespaces().await?)
    }

    /// Summed from the per-namespace counts the driver already reports, rather
    /// than from `list(None, None, None).len()`: the two answer the same
    /// question and only one of them pulls every entry's content across the bus
    /// to do it.
    async fn count(&self) -> anyhow::Result<usize> {
        let summaries = self.provider.namespaces().await?;
        Ok(summaries.iter().map(|summary| summary.count).sum())
    }

    /// `Degraded` counts as healthy here because the trait asks whether the
    /// backend is "reachable and able to serve requests" — a degraded driver
    /// is both. Only `Down` is false.
    async fn health_check(&self) -> bool {
        !matches!(self.provider.health().await, MemoryHealth::Down { .. })
    }

    /// The typed answer, so callers get the driver's own reason string instead
    /// of falling back to the boolean above.
    async fn health_probe(&self) -> Option<MemoryHealth> {
        Some(self.provider.health().await)
    }
}

#[derive(Debug, Deserialize)]
pub struct CaptureParams {
    pub experience: AgentExperience,
}

#[derive(Debug, Deserialize, Default)]
pub struct RetrieveParams {
    pub query: String,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub entrypoint: Option<String>,
    #[serde(default)]
    pub max_hits: Option<usize>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ListParams {
}

#[derive(Debug, Deserialize)]
pub struct DismissParams {
    pub id: String,
}

#[derive(Debug, Serialize)]
pub struct DismissResult {
    pub id: String,
    pub dismissed: bool,
}

async fn open_store() -> Result<AgentExperienceStore, String> {
    let config = Config::load_or_init()
        .await
        .map_err(|e| format!("load config: {e}"))?;
    open_store_in_subdir(&config, "memory").await
}

async fn open_store_in_subdir(
    config: &Config,
    memory_subdir: &str,
) -> Result<AgentExperienceStore, String> {
    let memory = DriverMemory::for_subtree(config, memory_subdir)
        .map_err(|e| format!("open agent experience store '{memory_subdir}': {e}"))?;
    Ok(AgentExperienceStore::new(memory))
}

async fn open_query_stores() -> Result<Vec<AgentExperienceStore>, String> {
    Ok(vec![open_store().await?])
}

pub async fn capture(params: CaptureParams) -> Result<RpcOutcome<AgentExperience>, String> {
    let store = open_store().await?;
    let stored = store.put(params.experience).await?;
    Ok(RpcOutcome::single_log(stored, "agent experience captured"))
}

pub async fn retrieve(params: RetrieveParams) -> Result<RpcOutcome<Vec<ExperienceHit>>, String> {
    let stores = open_query_stores().await?;
    let max_hits = params.max_hits.unwrap_or(5);
    let query = ExperienceQuery {
        query: params.query,
        tools: params.tools,
        tags: params.tags,
        agent_id: params.agent_id,
        entrypoint: params.entrypoint,
        max_hits,
    };
    let hits = retrieve_across_stores(&stores, query).await?;
    Ok(RpcOutcome::single_log(hits, "agent experiences retrieved"))
}

pub async fn list(params: ListParams) -> Result<RpcOutcome<Vec<AgentExperience>>, String> {
    let stores = open_query_stores().await?;
    let mut by_id: BTreeMap<String, AgentExperience> = BTreeMap::new();
    for store in stores {
        for experience in store.list().await? {
            let id = experience.id.clone();
            match by_id.get(&id) {
                Some(existing) if existing.updated_at_ms >= experience.updated_at_ms => {}
                _ => {
                    by_id.insert(id, experience);
                }
            }
        }
    }
    let mut experiences: Vec<_> = by_id.into_values().collect();
    experiences.sort_by(|a, b| {
        b.updated_at_ms
            .cmp(&a.updated_at_ms)
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(RpcOutcome::single_log(
        experiences,
        "agent experiences listed",
    ))
}

pub async fn dismiss(params: DismissParams) -> Result<RpcOutcome<DismissResult>, String> {
    let stores = open_query_stores().await?;
    let mut dismissed = false;
    for store in stores {
        dismissed |= store
            .dismiss(&params.id)
            .await?;
    }
    Ok(RpcOutcome::single_log(
        DismissResult {
            id: params.id,
            dismissed,
        },
        "agent experience dismissed",
    ))
}

#[cfg(test)]
#[path = "ops_tests.rs"]
mod tests;
