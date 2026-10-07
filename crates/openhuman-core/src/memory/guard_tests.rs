use super::*;

use tinymemory_api::conformance::ReferenceEngine;
use tinymemory_api::{MemoryMeta, MetaFilter};

const SECRET: &str = "sk-ant-api03-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

async fn texts(engine: &ReferenceEngine) -> Vec<String> {
    engine
        .list(ListRequest {
            filter: MetaFilter::default(),
            limit: 50,
            cursor: None,
        })
        .await
        .unwrap()
        .items
        .into_iter()
        .map(|hit| hit.text)
        .collect()
}

#[tokio::test]
async fn every_write_path_is_scrubbed() {
    let reference = Arc::new(ReferenceEngine::new());
    let guarded = ScrubbingEngine::wrap(reference.clone());
    let item =
        |text: &str| StoreItem::document(format!("{text} key={SECRET}"), MemoryMeta::default());

    guarded.store(item("one")).await.unwrap();
    guarded
        .store_with(item("two"), WriteOptions::accepted())
        .await
        .unwrap();
    guarded
        .store_many(vec![item("three"), item("four")])
        .await
        .unwrap();

    let stored = texts(&reference).await;
    assert_eq!(stored.len(), 4);
    assert!(
        stored.iter().all(|text| !text.contains(SECRET)),
        "a secret reached the engine: {stored:?}"
    );
    assert_eq!(guarded.descriptor().id, reference.descriptor().id);
}
