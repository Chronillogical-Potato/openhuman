use std::sync::Arc;

use tinyagents_harness::tool::discover::DiscoveryRankMode;
use tinytools::{Bm25Ranker, ToolRanker};

use super::*;

/// The slots are process globals; serialise the tests that touch them.
fn guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[test]
fn default_policy_is_auto_with_no_ranker_and_top_three() {
    let _g = guard();
    clear_tool_ranker();
    apply_tool_search_config(&ToolSearchConfig::default());
    let policy = discovery_policy();
    assert!(policy.ranker.is_none());
    assert_eq!(policy.rank_mode, DiscoveryRankMode::Ranker);
    assert_eq!(policy.default_limit, 3);
    assert!(policy.enabled);
}

#[test]
fn settings_select_the_mode_and_clamp_top_k() {
    let _g = guard();
    let ranker: Arc<dyn ToolRanker> = Arc::new(Bm25Ranker);
    install_tool_ranker(ranker);
    for (setting, mode) in [
        ("bm25", DiscoveryRankMode::Bm25),
        ("compare", DiscoveryRankMode::Compare),
        ("jev", DiscoveryRankMode::Ranker),
        ("nonsense", DiscoveryRankMode::Ranker),
    ] {
        apply_tool_search_config(&ToolSearchConfig {
            ranker: setting.into(),
            top_k: 500,
        });
        let policy = discovery_policy();
        assert_eq!(policy.rank_mode, mode, "{setting}");
        assert_eq!(policy.default_limit, policy.max_limit, "{setting}");
        assert_eq!(policy.ranker.as_ref().map(|r| r.kind()), Some("bm25"));
    }
    clear_tool_ranker();
    apply_tool_search_config(&ToolSearchConfig::default());
}

#[tokio::test]
async fn overlap_ranker_ranks_by_token_overlap_and_names_its_kind() {
    let ranker = OverlapRanker;
    assert_eq!(ranker.kind(), "overlap");
    let candidates = vec![
        tinytools::RankCandidate::new("SLACK_SEND_MESSAGE", "SLACK_SEND_MESSAGE send message to a channel"),
        tinytools::RankCandidate::new("GMAIL_FETCH_EMAILS", "GMAIL_FETCH_EMAILS fetch emails from inbox"),
    ];
    let hits = ranker
        .rank("send a message to the channel", &tinytools::RankContext::empty(), &candidates, 3)
        .await
        .unwrap();
    assert_eq!(hits.first().map(|h| h.key.as_str()), Some("SLACK_SEND_MESSAGE"));
    assert!(ranker
        .rank(" ", &tinytools::RankContext::empty(), &candidates, 3)
        .await
        .is_err());
}
