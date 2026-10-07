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

#[test]
fn a_timed_call_reports_ok_or_the_error_code() {
    assert_eq!(outcome(&Ok::<(), _>(())), "ok");
    let refused: Result<()> = Err(tinymemory_api::Error::Unauthorized("bad key".into()));
    assert_eq!(outcome(&refused), "UNAUTHORIZED");
}

#[tokio::test]
async fn every_read_passes_through_the_timer_unchanged() {
    let reference = Arc::new(ReferenceEngine::new());
    let guarded = ScrubbingEngine::wrap(reference.clone());
    let id = guarded
        .store(StoreItem::document("oolong tea", MemoryMeta::default()))
        .await
        .unwrap()
        .id;
    let reach = tinymemory_api::Reach::subtree(tinymemory_api::Namespace::ROOT);

    assert_eq!(guarded.health().await, reference.health().await);
    assert_eq!(
        guarded
            .recall(RecallRequest::new("oolong", 5))
            .await
            .unwrap(),
        reference
            .recall(RecallRequest::new("oolong", 5))
            .await
            .unwrap()
    );
    let fetch = || FetchRequest::new("oolong", tinymemory_api::FetchMode::Keyword, 5);
    assert_eq!(
        guarded.fetch(fetch()).await.unwrap(),
        reference.fetch(fetch()).await.unwrap()
    );
    let get = || GetRequest {
        ids: vec![id.clone()],
        reach: None,
    };
    assert_eq!(guarded.get(get()).await.unwrap().len(), 1);
    let explore = || ExploreRequest::new(tinymemory_api::Facet::Kind, 5);
    assert_eq!(
        guarded.explore(explore()).await.ok(),
        reference.explore(explore()).await.ok()
    );
    assert_eq!(
        guarded
            .consolidate(ConsolidateRequest::new(reach.clone()))
            .await
            .ok(),
        reference
            .consolidate(ConsolidateRequest::new(reach.clone()))
            .await
            .ok()
    );
    assert_eq!(
        guarded
            .beliefs(BeliefsRequest::new(reach.clone(), 5))
            .await
            .ok(),
        reference.beliefs(BeliefsRequest::new(reach, 5)).await.ok()
    );
    assert_eq!(
        guarded
            .forget(ForgetTarget::Ids(vec![id]))
            .await
            .unwrap()
            .forgotten,
        1
    );
}

#[test]
fn scrubbing_keeps_the_page_breaks_a_document_is_chunked_by() {
    let body = format!("Page one key={SECRET}\u{c}Page two.\u{c}Page three.");
    let scrubbed = scrub(StoreItem::document(body, MemoryMeta::default()));
    let StoreItem::Document {
        body: tinymemory_api::DocumentBody::Text(text),
        ..
    } = scrubbed
    else {
        panic!("a document");
    };
    assert!(!text.contains(SECRET), "{text}");
    assert_eq!(text.matches('\u{c}').count(), 2, "{text:?}");
    assert!(text.ends_with("\u{c}Page two.\u{c}Page three."), "{text:?}");
}

#[tokio::test]
async fn export_and_erase_reach_the_wrapped_engine() {
    let reference = Arc::new(ReferenceEngine::new());
    let guarded = ScrubbingEngine::wrap(reference.clone());
    let namespace: tinymemory_api::Namespace = "agent:a".parse().unwrap();
    guarded
        .store(StoreItem::document(
            "kept whole",
            MemoryMeta {
                namespace: namespace.clone(),
                ..MemoryMeta::default()
            },
        ))
        .await
        .unwrap();
    let page = guarded
        .export(ListRequest::new(MetaFilter::default(), 10))
        .await
        .expect("export is forwarded, not refused by the wrapper");
    assert_eq!(page.items.len(), 1);
    let report = guarded
        .erase(EraseRequest::new(tinymemory_api::Reach::exact(namespace)))
        .await
        .expect("erase is forwarded, not refused by the wrapper");
    assert_eq!(report.erased_scopes, 1);
    assert!(texts(&reference).await.is_empty());
}
