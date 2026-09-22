//! [`TinyHumansJevRanker`]: a `ToolRanker` that resolves the process's
//! TinyHumans credential on every search and ranks through Jev.

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    sync::Mutex,
};

use std::{future::Future, pin::Pin, sync::Arc};

use openhuman_core::agent::tinyagents::discovery::EmbeddingToolRanker;
use openhuman_core::api::config::effective_backend_api_url;
use openhuman_core::config::Config;
use openhuman_core::security::credentials::session_support::resolve_backend_credential;
use tinytools::{RankCandidate, RankContext, RankError, RankHit, ToolRanker};
use tinyjevclient::{Client, ClientConfig};
use tinytools_jev::{JevRanker, JevRankerConfig, JevStrategy};

use super::evaluator::TinyJevEvaluator;

/// How the ranker reads the config a search runs under. The default is the
/// core's own read path (the embedder's config when one is bound, else the
/// process-global load); a test hands in a fixed one.
pub type ConfigLoader = Arc<
    dyn Fn() -> Pin<Box<dyn Future<Output = Result<Config, String>> + Send>> + Send + Sync,
>;

/// A [`JevRanker`] bound to whichever credential and backend the process has
/// at search time.
pub struct TinyHumansJevRanker {
    config: JevRankerConfig,
    load_config: ConfigLoader,
    cached: Mutex<Option<Cached>>,
}

struct Cached {
    fingerprint: u64,
    ranker: JevRanker,
    /// The retriever inside `ranker`, kept so its catalogue embeddings
    /// survive a credential change.
    retriever: Arc<dyn ToolRanker>,
}

impl std::fmt::Debug for TinyHumansJevRanker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TinyHumansJevRanker")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl Default for TinyHumansJevRanker {
    fn default() -> Self {
        Self::new()
    }
}

impl TinyHumansJevRanker {
    /// A ranker with the product defaults: family-then-decide (the
    /// evaluator picks the toolkit or pack, then the tool), the process's
    /// embedding provider as the retriever for any family too large for one
    /// choice, a 3 s deadline per evaluation.
    pub fn new() -> Self {
        Self::with_config(JevRankerConfig::new().with_strategy(JevStrategy::FamilyThenDecide))
    }

    /// A ranker with an explicit `tinytools-jev` configuration.
    pub fn with_config(config: JevRankerConfig) -> Self {
        Self {
            config,
            load_config: Arc::new(|| {
                Box::pin(openhuman_core::config::ops::load_config_with_timeout())
            }),
            cached: Mutex::new(None),
        }
    }

    /// Reads the config through `loader` instead of the core's read path.
    pub fn with_config_loader(mut self, loader: ConfigLoader) -> Self {
        self.load_config = loader;
        self
    }

    /// The `JevRanker` for the current credential and backend, built or
    /// reused.
    ///
    /// Reads the config and credential fresh each time: the cost is a config
    /// load, which a `tool_search` (one model round trip plus a network call)
    /// dwarfs, and the benefit is that sign-in, sign-out and a backend URL
    /// change are all honoured by the next search.
    async fn current(&self) -> Result<JevRanker, RankError> {
        let config = (self.load_config)()
            .await
            .map_err(|error| RankError::Backend {
                reason: format!("config unavailable: {error}"),
            })?;
        let credential = resolve_backend_credential(&config).map_err(|reason| {
            // The message names what is missing, never a secret.
            RankError::Backend {
                reason: format!("no TinyHumans credential ({reason})"),
            }
        })?;
        let base_url = effective_backend_api_url(&config.api_url);
        let fingerprint = fingerprint(credential.secret(), &base_url);

        let mut cached = self
            .cached
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(entry) = cached.as_ref().filter(|entry| entry.fingerprint == fingerprint) {
            return Ok(entry.ranker.clone());
        }
        let mut client_config = ClientConfig::tinyhumans_openrouter(credential.into_secret());
        client_config.base_url = base_url.clone();
        let client = Client::new(client_config)
            .map_err(|error| RankError::invalid_input(error.to_string()))?;
        let evaluator: Arc<dyn tinytools_jev::JevEvaluator> =
            Arc::new(TinyJevEvaluator::new(client));
        // The retriever is the process's embedding provider when it can
        // embed (the same one memory recall uses), so a family larger than
        // one Jev Choice is cut by meaning, not by shared words. Reused
        // across rebuilds so the catalogue is embedded once per process.
        let retriever: Arc<dyn ToolRanker> = match cached.as_ref() {
            Some(entry) => entry.retriever.clone(),
            None => retriever_for(&config),
        };
        let ranker = JevRanker::new(
            evaluator,
            self.config.clone().with_retriever(retriever.clone()),
        );
        log::info!(
            "[tool-search] jev ranker bound to backend {} ({})",
            openhuman_core::util::redact::redact_url_for_log(&base_url),
            if fingerprint_changed(cached.as_ref(), fingerprint) {
                "credential or backend changed"
            } else {
                "first search"
            }
        );
        *cached = Some(Cached {
            fingerprint,
            ranker: ranker.clone(),
            retriever,
        });
        Ok(ranker)
    }
}

/// The semantic retriever for `config`'s embedding provider, or BM25 when
/// the provider cannot embed (`none`, or a managed provider with no route).
fn retriever_for(config: &Config) -> Arc<dyn ToolRanker> {
    let provider = openhuman_core::inference::embedding_host::default_embedding_provider_with_config(
        config,
    );
    if !EmbeddingToolRanker::provider_is_usable(provider.as_ref()) {
        log::info!(
            "[tool-search] embedding provider `{}` cannot embed; retrieving with bm25",
            provider.name()
        );
        return Arc::new(tinytools::Bm25Ranker);
    }
    log::info!(
        "[tool-search] retrieving with embeddings ({} / {})",
        provider.name(),
        provider.model_id()
    );
    Arc::new(
        EmbeddingToolRanker::new(provider).with_disk_cache(
            config
                .workspace_dir
                .join("cache")
                .join("tool_search_embeddings.json"),
        ),
    )
}

fn fingerprint(secret: &str, base_url: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    secret.hash(&mut hasher);
    base_url.hash(&mut hasher);
    hasher.finish()
}

fn fingerprint_changed(cached: Option<&Cached>, fingerprint: u64) -> bool {
    cached.is_some_and(|entry| entry.fingerprint != fingerprint)
}

#[async_trait::async_trait]
impl ToolRanker for TinyHumansJevRanker {
    fn kind(&self) -> &'static str {
        JevRanker::KIND
    }

    async fn rank(
        &self,
        intent: &str,
        context: &RankContext,
        candidates: &[RankCandidate],
        limit: usize,
    ) -> Result<Vec<RankHit>, RankError> {
        let ranker = self.current().await?;
        let ranking = ranker
            .rank_detailed(intent, context, candidates, limit)
            .await?;
        log::debug!(
            "[tool-search] jev ranked {} of {} shown (families={:?} choice_confidence={:.2} needs_tool={:?} none={:.2} latency_ms={} attempts={} input_tokens={:?})",
            ranking.hits.len(),
            ranking.shortlisted,
            ranking.families,
            ranking.choice_confidence,
            ranking.needs_tool,
            ranking.none_probability,
            ranking.latency.as_millis(),
            ranking.attempts,
            ranking.input_tokens,
        );
        Ok(ranking.hits)
    }
}

#[cfg(test)]
#[path = "ranker_tests.rs"]
mod tests;
