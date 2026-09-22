use tinytools::{RankCandidate, RankContext, RankError, ToolRanker};

use super::*;

#[test]
fn kind_is_jev_and_debug_hides_the_client() {
    let ranker = TinyHumansJevRanker::new();
    assert_eq!(ranker.kind(), "jev");
    let debug = format!("{ranker:?}");
    assert!(debug.contains("TinyHumansJevRanker"));
    assert!(!debug.contains("api_key"));
}

#[test]
fn fingerprint_changes_with_secret_or_base_url() {
    let a = fingerprint("secret", "https://api.tinyhumans.ai");
    assert_eq!(a, fingerprint("secret", "https://api.tinyhumans.ai"));
    assert_ne!(a, fingerprint("other", "https://api.tinyhumans.ai"));
    assert_ne!(a, fingerprint("secret", "https://staging.tinyhumans.ai"));
    assert!(!fingerprint_changed(None, a));
}

/// Without a credential the ranker must answer with a `Backend` error the
/// harness turns into a BM25 fallback — never a panic, never a network call.
#[tokio::test]
async fn no_credential_is_a_backend_error_naming_the_gap() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config = Config {
        workspace_dir: tmp.path().join("workspace"),
        action_dir: tmp.path().join("workspace"),
        config_path: tmp.path().join("config.toml"),
        ..Config::default()
    };
    let ranker = TinyHumansJevRanker::new().with_config_loader(Arc::new(move || {
        let config = config.clone();
        Box::pin(async move { Ok(config) })
    }));
    let err = ranker
        .rank(
            "send a message",
            &RankContext::empty(),
            &[RankCandidate::new("a", "a send a message")],
            3,
        )
        .await
        .expect_err("no credential must fail");
    match err {
        RankError::Backend { reason } => {
            assert!(reason.contains("no TinyHumans credential"), "{reason}");
        }
        other => panic!("expected a backend error, got {other}"),
    }
}

/// A config whose embedding provider is `none` disables the Jev search
/// outright — the harness's BM25 answers — rather than quietly retrieving
/// lexically.
#[test]
fn no_embedding_provider_disables_the_search() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut config = Config {
        workspace_dir: tmp.path().join("workspace"),
        action_dir: tmp.path().join("workspace"),
        config_path: tmp.path().join("config.toml"),
        ..Config::default()
    };
    config.memory.embedding_provider = "none".into();
    let err = retriever_for(&config).err().map(|e| e.to_string());
    assert!(
        err.as_deref()
            .is_some_and(|e| e.contains("no usable embedding provider")),
        "{err:?}"
    );
}
