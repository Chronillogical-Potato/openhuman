//! [`TinyHumansJevRanker`]: a `ToolRanker` that resolves the process's
//! TinyHumans credential on every search and ranks through Jev.

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    sync::Mutex,
};

use std::{future::Future, pin::Pin, sync::Arc};

use openhuman_core::api::config::effective_backend_api_url;
use openhuman_core::config::Config;
use openhuman_core::security::credentials::session_support::resolve_backend_credential;
use tinytools::{RankCandidate, RankContext, RankError, RankHit, ToolRanker};
use tinytools_jev::{ClientConfig, JevRanker, JevRankerConfig};

/// How the ranker reads the config a search runs under. The default is the
/// core's own read path (the embedder's config when one is bound, else the
/// process-global load); a test hands in a fixed one.
pub type ConfigLoader =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<Config, String>> + Send>> + Send + Sync>;

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
    /// A ranker with `tinytools-jev`'s defaults: BM25 retrieval to 20, one
    /// Jev decision, a 3 s deadline.
    pub fn new() -> Self {
        Self::with_config(JevRankerConfig::new())
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
        if let Some(entry) = cached
            .as_ref()
            .filter(|entry| entry.fingerprint == fingerprint)
        {
            return Ok(entry.ranker.clone());
        }
        let mut client = ClientConfig::tinyhumans_openrouter(credential.into_secret());
        client.base_url = base_url.clone();
        let ranker = JevRanker::from_config(client, self.config.clone())?;
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
        });
        Ok(ranker)
    }
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
            "[tool-search] jev ranked {} of {} shortlisted (choice_confidence={:.2} needs_tool={:?} none={:.2} latency_ms={} attempts={} input_tokens={:?})",
            ranking.hits.len(),
            ranking.shortlisted,
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
