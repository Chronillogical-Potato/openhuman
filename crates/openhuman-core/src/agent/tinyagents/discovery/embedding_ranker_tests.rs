use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tinytools::{RankCandidate, RankContext, ToolRanker};

use super::*;

/// Embeds a text as a bag of three hand-picked words, so similarity is
/// deterministic and readable.
struct BagEmbedder {
    calls: AtomicUsize,
}

#[async_trait::async_trait]
impl EmbeddingProvider for BagEmbedder {
    fn name(&self) -> &str {
        "bag"
    }
    fn model_id(&self) -> &str {
        "bag-v1"
    }
    fn dimensions(&self) -> usize {
        3
    }
    fn signature(&self) -> String {
        "provider=bag;model=bag-v1;dims=3".into()
    }
    async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(texts
            .iter()
            .map(|t| {
                let t = t.to_ascii_lowercase();
                vec![
                    f32::from(u8::from(t.contains("message") || t.contains("ping"))),
                    f32::from(u8::from(t.contains("email") || t.contains("mail"))),
                    f32::from(u8::from(t.contains("file"))),
                ]
            })
            .collect())
    }
}

fn candidates() -> Vec<RankCandidate> {
    vec![
        RankCandidate::new("SLACK_SEND_MESSAGE", "send a message to a channel")
            .with_family("slack"),
        RankCandidate::new("GMAIL_SEND_EMAIL", "send an email").with_family("gmail"),
        RankCandidate::new("file_read", "read a file"),
    ]
}

#[tokio::test]
async fn ranks_by_cosine_and_embeds_the_catalogue_once() {
    let embedder = Arc::new(BagEmbedder {
        calls: AtomicUsize::new(0),
    });
    let ranker = EmbeddingToolRanker::new(embedder.clone());
    assert_eq!(ranker.kind(), "embedding");

    let hits = ranker
        .rank("ping alex", &RankContext::empty(), &candidates(), 2)
        .await
        .unwrap();
    assert_eq!(hits[0].key, "SLACK_SEND_MESSAGE");
    assert!(hits[0].confidence.is_none());
    assert_eq!(hits.len(), 2);
    // One batch for the catalogue plus one for the intent.
    assert_eq!(embedder.calls.load(Ordering::SeqCst), 2);

    let hits = ranker
        .rank("mail the report", &RankContext::empty(), &candidates(), 1)
        .await
        .unwrap();
    assert_eq!(hits[0].key, "GMAIL_SEND_EMAIL");
    // Only the intent was embedded this time.
    assert_eq!(embedder.calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn disk_cache_round_trips_and_is_keyed_by_signature() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("cache").join("tool_search_embeddings.json");
    let embedder = Arc::new(BagEmbedder {
        calls: AtomicUsize::new(0),
    });
    let ranker = EmbeddingToolRanker::new(embedder.clone()).with_disk_cache(path.clone());
    ranker
        .rank("ping", &RankContext::empty(), &candidates(), 1)
        .await
        .unwrap();
    assert!(path.exists());

    let embedder2 = Arc::new(BagEmbedder {
        calls: AtomicUsize::new(0),
    });
    let warm = EmbeddingToolRanker::new(embedder2.clone()).with_disk_cache(path.clone());
    warm.rank("ping", &RankContext::empty(), &candidates(), 1)
        .await
        .unwrap();
    assert_eq!(
        embedder2.calls.load(Ordering::SeqCst),
        1,
        "a warm cache embeds only the intent"
    );
}

#[tokio::test]
async fn empty_intent_is_rejected_and_none_provider_is_unusable() {
    let ranker = EmbeddingToolRanker::new(Arc::new(BagEmbedder {
        calls: AtomicUsize::new(0),
    }));
    assert!(ranker
        .rank("  ", &RankContext::empty(), &candidates(), 1)
        .await
        .is_err());
    let none = crate::inference::embedding_host::TinyInferenceEmbeddingProvider::new(
        tinyinference_embeddings::NoopEmbeddingModel,
    );
    assert!(!EmbeddingToolRanker::provider_is_usable(&none));
}

/// A tool that appears later — a newly connected toolkit's actions, a
/// rewritten description — is embedded on its own; the rest is a cache hit.
#[tokio::test]
async fn a_new_or_changed_tool_is_embedded_incrementally() {
    let embedder = Arc::new(BagEmbedder {
        calls: AtomicUsize::new(0),
    });
    let ranker = EmbeddingToolRanker::new(embedder.clone());
    ranker
        .rank("ping", &RankContext::empty(), &candidates(), 1)
        .await
        .unwrap();
    assert_eq!(
        embedder.calls.load(Ordering::SeqCst),
        2,
        "catalogue + intent"
    );

    let mut grown = candidates();
    grown.push(RankCandidate::new("NOTION_CREATE_PAGE", "create a page").with_family("notion"));
    grown[2] = RankCandidate::new("file_read", "read a file from disk");
    ranker
        .rank("ping", &RankContext::empty(), &grown, 1)
        .await
        .unwrap();
    // One batch for the two unseen texts (the new tool and the changed one),
    // plus the intent — never the whole catalogue again.
    assert_eq!(embedder.calls.load(Ordering::SeqCst), 4);
    assert_eq!(
        ranker.cache.read().unwrap().len(),
        5,
        "old and new descriptions both cached; a stale entry is harmless"
    );
}
