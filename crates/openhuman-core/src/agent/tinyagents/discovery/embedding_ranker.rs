//! [`EmbeddingToolRanker`]: the semantic retriever for `tool_search`.
//!
//! A lexical retriever loses every paraphrase — "ping alex" never overlaps
//! `SLACK_SEND_MESSAGE send a message` — and a decision model can only pick
//! from what the retriever hands it. Measured on the orchestrator's catalogue
//! plus nine Composio toolkits (1,215 tools), BM25 recall@20 was 70%, and
//! that ceiling capped Jev at 67% top-3. This ranker embeds each tool's
//! summary once with the process's configured embedding provider (the same
//! one memory recall uses), embeds the intent per search, and ranks by cosine
//! similarity, so recall follows meaning rather than shared words. It is the
//! `retriever` inside `tinytools_jev::JevRanker` for every catalogue larger
//! than one Jev `Choice`.
//!
//! Catalogue embeddings are cached in memory by content hash and, when a
//! cache path is given, on disk keyed by the provider's signature, so a cold
//! process pays for the catalogue once and every later search embeds only
//! the intent (one provider call).

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use tinytools::{RankCandidate, RankContext, RankError, RankHit, ToolRanker};

use crate::inference::embedding_host::EmbeddingProvider;

/// Texts per embedding request. The managed provider accepts far more, but a
/// bounded batch keeps one request under any body cap and lets a partial
/// failure cost one batch, not the catalogue.
const EMBED_BATCH: usize = 64;

/// Ranks tools by cosine similarity between the intent and each tool's
/// summary, with catalogue embeddings cached.
pub struct EmbeddingToolRanker {
    provider: Arc<dyn EmbeddingProvider>,
    cache: RwLock<HashMap<u64, Vec<f32>>>,
    cache_path: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Default)]
struct DiskCache {
    signature: String,
    entries: HashMap<u64, Vec<f32>>,
}

impl std::fmt::Debug for EmbeddingToolRanker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbeddingToolRanker")
            .field("provider", &self.provider.name())
            .field("model", &self.provider.model_id())
            .field("cache_path", &self.cache_path)
            .finish_non_exhaustive()
    }
}

impl EmbeddingToolRanker {
    /// The stable [`ToolRanker::kind`] of this ranker.
    pub const KIND: &'static str = "embedding";

    /// A ranker over `provider` with an in-memory cache only.
    pub fn new(provider: Arc<dyn EmbeddingProvider>) -> Self {
        Self {
            provider,
            cache: RwLock::new(HashMap::new()),
            cache_path: None,
        }
    }

    /// A ranker whose catalogue embeddings also persist at `path`, keyed by
    /// the provider's signature so a model change invalidates them. A
    /// missing or unreadable file is an empty cache, never an error.
    pub fn with_disk_cache(mut self, path: PathBuf) -> Self {
        let loaded = std::fs::read(&path)
            .ok()
            .and_then(|raw| serde_json::from_slice::<DiskCache>(&raw).ok())
            .filter(|disk| disk.signature == self.provider.signature());
        if let Some(disk) = loaded {
            tracing::debug!(
                entries = disk.entries.len(),
                path = %path.display(),
                "[tool-search] loaded embedding cache"
            );
            *self.cache.write().unwrap_or_else(|p| p.into_inner()) = disk.entries;
        }
        self.cache_path = Some(path);
        self
    }

    /// Whether `provider` can embed at all. The `none` provider embeds
    /// nothing and would rank everything at zero.
    pub fn provider_is_usable(provider: &dyn EmbeddingProvider) -> bool {
        provider.dimensions() > 0 && provider.name() != "none"
    }

    fn key(candidate: &RankCandidate) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        candidate.summary.hash(&mut hasher);
        candidate.family.hash(&mut hasher);
        hasher.finish()
    }

    fn text(candidate: &RankCandidate) -> String {
        match &candidate.family {
            Some(family) => format!("{} ({family})", candidate.summary),
            None => candidate.summary.clone(),
        }
    }

    /// Embeds every candidate not already cached, in batches.
    async fn ensure_cached(&self, candidates: &[RankCandidate]) -> Result<(), RankError> {
        let missing: Vec<(u64, String)> = {
            let cache = self.cache.read().unwrap_or_else(|p| p.into_inner());
            let mut seen = std::collections::HashSet::new();
            candidates
                .iter()
                .map(|c| (Self::key(c), c))
                .filter(|(k, _)| !cache.contains_key(k) && seen.insert(*k))
                .map(|(k, c)| (k, Self::text(c)))
                .collect()
        };
        if missing.is_empty() {
            return Ok(());
        }
        tracing::info!(
            missing = missing.len(),
            provider = self.provider.name(),
            "[tool-search] embedding catalogue entries"
        );
        for batch in missing.chunks(EMBED_BATCH) {
            let texts: Vec<&str> = batch.iter().map(|(_, t)| t.as_str()).collect();
            let vectors =
                self.provider
                    .embed(&texts)
                    .await
                    .map_err(|error| RankError::Backend {
                        reason: format!("embedding failed: {error:#}"),
                    })?;
            if vectors.len() != batch.len() {
                return Err(RankError::Backend {
                    reason: format!(
                        "embedding returned {} vectors for {} texts",
                        vectors.len(),
                        batch.len()
                    ),
                });
            }
            let mut cache = self.cache.write().unwrap_or_else(|p| p.into_inner());
            for ((key, _), vector) in batch.iter().zip(vectors) {
                cache.insert(*key, vector);
            }
        }
        self.persist();
        Ok(())
    }

    fn persist(&self) {
        let Some(path) = &self.cache_path else {
            return;
        };
        let disk = DiskCache {
            signature: self.provider.signature(),
            entries: self.cache.read().unwrap_or_else(|p| p.into_inner()).clone(),
        };
        let write = || -> std::io::Result<()> {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let tmp = path.with_extension("json.tmp");
            std::fs::write(&tmp, serde_json::to_vec(&disk)?)?;
            std::fs::rename(&tmp, path)
        };
        if let Err(error) = write() {
            tracing::warn!(
                path = %path.display(),
                error = %error,
                "[tool-search] could not persist embedding cache"
            );
        }
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f64 {
    let (mut dot, mut na, mut nb) = (0.0_f64, 0.0_f64, 0.0_f64);
    for (x, y) in a.iter().zip(b) {
        dot += f64::from(*x) * f64::from(*y);
        na += f64::from(*x) * f64::from(*x);
        nb += f64::from(*y) * f64::from(*y);
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

#[async_trait::async_trait]
impl ToolRanker for EmbeddingToolRanker {
    fn kind(&self) -> &'static str {
        Self::KIND
    }

    async fn rank(
        &self,
        intent: &str,
        _context: &RankContext,
        candidates: &[RankCandidate],
        limit: usize,
    ) -> Result<Vec<RankHit>, RankError> {
        let intent = intent.trim();
        if intent.is_empty() {
            return Err(RankError::InvalidInput {
                reason: "intent is empty".to_owned(),
            });
        }
        if candidates.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        self.ensure_cached(candidates).await?;
        let query = self
            .provider
            .embed(&[intent])
            .await
            .map_err(|error| RankError::Backend {
                reason: format!("embedding failed: {error:#}"),
            })?
            .into_iter()
            .next()
            .ok_or_else(|| RankError::Backend {
                reason: "embedding returned no vector for the intent".to_owned(),
            })?;
        let cache = self.cache.read().unwrap_or_else(|p| p.into_inner());
        let mut scored: Vec<RankHit> = candidates
            .iter()
            .filter_map(|c| {
                cache
                    .get(&Self::key(c))
                    .map(|v| RankHit::new(c.key.clone(), cosine(&query, v)))
            })
            .collect();
        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.key.cmp(&b.key))
        });
        scored.truncate(limit);
        Ok(scored)
    }
}

#[cfg(test)]
#[path = "embedding_ranker_tests.rs"]
mod tests;
