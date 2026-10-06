//! The memory facade against TinyMemory's in-memory reference engine: one
//! runtime, two tenant roots, and the guarantee that neither reaches the
//! other.
//!
//! One `#[test]` because a runtime claims a process-wide slot, and the host
//! engine it installs is process-wide too.

mod common;

use std::sync::Arc;

use common::{offline_config, runtime};
use openhuman_embed::memory::api::conformance::ReferenceEngine;
use openhuman_embed::memory::api::{
    ItemKind, LearningKind, MemoryEngine, MemoryMeta, Namespace, StoreItem,
};
use openhuman_embed::memory::{ItemsQuery, LearnParams};
use openhuman_embed::{Runtime, Workspace};

fn learning(text: &str) -> LearnParams {
    serde_json::from_value(serde_json::json!({ "text": text })).expect("learn params")
}

async fn store_at(engine: &ReferenceEngine, node: &str, text: &str) -> String {
    let meta = MemoryMeta {
        namespace: node.parse::<Namespace>().expect("node"),
        ..MemoryMeta::default()
    };
    engine
        .store(StoreItem::learning(text, LearningKind::Fact, 0.9, meta))
        .await
        .expect("store")
        .id
        .0
}

#[test]
fn a_tenant_root_confines_every_call() {
    runtime().block_on(async {
        tokio::spawn(async {
            let engine = Arc::new(ReferenceEngine::new());
            let rt = Runtime::builder()
                .workspace(Workspace::Ephemeral)
                .config(offline_config())
                .memory_engine(engine.clone())
                .build()
                .await
                .expect("runtime");

            let acme = rt.memory("team:acme").expect("acme");
            let globex = rt.memory("team:globex").expect("globex");
            assert!(acme.status().on, "the installed engine binds");
            assert!(rt.memory("").is_err(), "the store root is no tenant");

            // Learnings land at the tenant root, whatever the caller's meta says.
            let mut params = learning("Acme ships on Fridays");
            params.meta = Some(MemoryMeta {
                namespace: "team:globex".parse().unwrap(),
                ..MemoryMeta::default()
            });
            let learned = acme.learn(params).await.expect("learn");
            let got = acme.get(vec![learned.id.clone()]).await.expect("get");
            assert_eq!(got.len(), 1);
            assert_eq!(got[0].meta.namespace.to_string(), "team:acme");
            assert!(globex
                .get(vec![learned.id.clone()])
                .await
                .unwrap()
                .is_empty());

            // One agent's node, listed on its own.
            let ceo = store_at(&engine, "team:acme/agent:ceo", "the CEO prefers email").await;
            let cfo = store_at(
                &engine,
                "team:acme/agent:cfo",
                "the CFO closes books monthly",
            )
            .await;
            let ceo_items = acme
                .list(ItemsQuery {
                    agent_id: Some("ceo".into()),
                    ..ItemsQuery::default()
                })
                .await
                .expect("list ceo");
            let ids: Vec<_> = ceo_items.items.iter().map(|hit| hit.id.0.clone()).collect();
            assert_eq!(ids, vec![ceo.clone()]);

            let all = acme.list(ItemsQuery::default()).await.expect("list all");
            assert_eq!(all.items.len(), 3);
            let learnings = acme
                .list(ItemsQuery {
                    kinds: vec![ItemKind::Learning],
                    ..ItemsQuery::default()
                })
                .await
                .unwrap();
            assert_eq!(learnings.items.len(), 3);
            assert!(globex
                .list(ItemsQuery::default())
                .await
                .unwrap()
                .items
                .is_empty());

            // Another tenant cannot forget acme's items by id.
            let forgotten = globex
                .forget(vec![ceo.clone(), learned.id.clone()])
                .await
                .unwrap();
            assert_eq!(forgotten.forgotten, 0);
            assert_eq!(acme.get(vec![ceo.clone()]).await.unwrap().len(), 1);

            // Forgetting one agent leaves its teammates and the shared learnings.
            assert_eq!(acme.forget_agent("ceo").await.expect("forget agent"), 1);
            assert!(acme.get(vec![ceo]).await.unwrap().is_empty());
            assert_eq!(acme.get(vec![cfo]).await.unwrap().len(), 1);
            assert_eq!(acme.get(vec![learned.id]).await.unwrap().len(), 1);

            assert!(openhuman_embed::memory::clear_host_engine());
        })
        .await
        .expect("memory facade test task");
    });
}
