use std::sync::Arc;

use tinymemory_api::conformance::ReferenceEngine;
use tinymemory_api::{LearningKind, ListRequest, MemoryEngine, MemoryMeta};

use super::*;

fn ns(raw: &str) -> Namespace {
    raw.parse().unwrap()
}

fn user_config(root: &str) -> Config {
    let mut config = Config::default();
    config.memory.root = Some(root.to_string());
    config
}

#[test]
fn only_saas_configs_with_a_root_are_confined() {
    assert_eq!(confinement_in(false, &user_config("user:u-a")), None);
    assert_eq!(confinement_in(true, &Config::default()), None);
    assert_eq!(
        confinement_in(true, &user_config("user:u-a")),
        Some(ns("user:u-a"))
    );
}

#[test]
fn reads_never_climb_out_of_the_root() {
    let root = ns("user:u-a");
    assert_eq!(clamp_reach(None, &root), Reach::subtree(root.clone()));
    assert_eq!(
        clamp_reach(Some(Reach::of(Namespace::ROOT)), &root),
        Reach::subtree(root.clone()),
        "the tree's root reads everyone"
    );
    assert_eq!(
        clamp_reach(Some(Reach::subtree(ns("user:u-b"))), &root),
        Reach::subtree(root.clone()),
        "another user's tree"
    );
    let inside = clamp_reach(Some(Reach::of(ns("user:u-a/agent:u-a"))), &root);
    assert_eq!(inside.at, ns("user:u-a/agent:u-a"));
    assert!(!inside.inherit, "ancestors would include the shared root");
}

#[test]
fn writes_land_inside_the_root() {
    let root = ns("user:u-a");
    let mut meta = MemoryMeta::default();
    meta.namespace = ns("user:u-b");
    let mut item = StoreItem::learning("x", LearningKind::Fact, 0.8, meta);
    clamp_item(&mut item, &root);
    assert_eq!(item.meta().namespace, root);

    let mut meta = MemoryMeta::default();
    meta.namespace = ns("user:u-a/agent:u-a");
    let mut item = StoreItem::learning("x", LearningKind::Fact, 0.8, meta);
    clamp_item(&mut item, &root);
    assert_eq!(item.meta().namespace, ns("user:u-a/agent:u-a"));
}

/// Two users on one shared engine: a clamped filter lists only the caller's
/// items, even when the caller asked for the whole tree.
#[tokio::test]
async fn a_shared_engine_keeps_users_apart_once_clamped() {
    let engine: Arc<dyn MemoryEngine> = Arc::new(ReferenceEngine::new());
    for (user, text) in [("user:u-a", "alice's fact"), ("user:u-b", "bob's fact")] {
        let mut item = StoreItem::learning(text, LearningKind::Fact, 0.8, MemoryMeta::default());
        clamp_item(&mut item, &ns(user));
        engine.store(item).await.unwrap();
    }
    let everyone = MetaFilter {
        reach: Some(Reach::subtree(Namespace::ROOT)),
        ..MetaFilter::default()
    };
    let unclamped = engine
        .list(ListRequest::new(everyone.clone(), 100))
        .await
        .unwrap();
    assert_eq!(unclamped.items.len(), 2, "unclamped, the tree root reads both");

    let clamped = engine
        .list(ListRequest::new(clamp_filter(everyone, &ns("user:u-a")), 100))
        .await
        .unwrap();
    let texts: Vec<_> = clamped
        .items
        .iter()
        .map(|hit| format!("{hit:?}"))
        .collect();
    assert_eq!(clamped.items.len(), 1, "{texts:?}");
    assert!(texts[0].contains("alice"), "{texts:?}");
}
