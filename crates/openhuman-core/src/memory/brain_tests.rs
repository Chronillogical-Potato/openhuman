use super::*;

use tinymemory_api::{ItemKind, MemoryMeta, MetaFilter};

use crate::memory::test_fixtures::{bind_reference, config_in, stored};

fn document(mime: Option<&str>, path: Option<&str>) -> StoreItem {
    let mut meta = MemoryMeta::default();
    meta.file_path = path.map(str::to_string);
    meta.agent_id = Some("someone".into());
    StoreItem::Document {
        title: None,
        body: tinymemory_api::DocumentBody::Text("body".into()),
        mime: mime.map(str::to_string),
        meta,
    }
}

#[test]
fn synced_items_are_filed_by_their_connector() {
    for (kind, target, want) in [
        (MemorySourceKind::Github, "o/r", BrainSource::Github),
        (MemorySourceKind::Link, "https://x", BrainSource::Web),
        (MemorySourceKind::Rss, "https://x/feed", BrainSource::Web),
        (MemorySourceKind::Composio, "Notion", BrainSource::Notion),
        // The Composio toolkit and the GitHub reader share one source.
        (MemorySourceKind::Composio, "github", BrainSource::Github),
        (
            MemorySourceKind::Composio,
            "gmail",
            BrainSource::Other("gmail".into()),
        ),
        // An alias files under the slug Composio itself uses.
        (
            MemorySourceKind::Composio,
            "google_drive",
            BrainSource::Other("googledrive".into()),
        ),
        // Local files, whatever their format, share one source.
        (MemorySourceKind::Folder, "/n", files_source()),
        (MemorySourceKind::File, "/n/deck/Q3.PDF", files_source()),
    ] {
        assert_eq!(brain_source(kind, target), want, "{kind:?} {target}");
    }
    assert_eq!(files_source().to_string(), "files");
}

#[test]
fn filing_moves_an_item_to_its_source_node_without_an_agent() {
    let team = MemoryLayout::new("team:acme".parse().unwrap()).unwrap();
    let filed = file_into(&team, &BrainSource::Pdf, document(None, None)).unwrap();
    assert_eq!(filed.meta().namespace.to_string(), "team:acme/source:pdf");
    assert_eq!(filed.meta().agent_id, None);
}

#[tokio::test]
async fn text_is_ingested_searched_counted_and_forgotten_per_source() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    let engine = bind_reference(&config);

    let ingested = ingest(
        &config,
        BrainIngestParams {
            path: None,
            text: Some("Refunds are issued within 14 days.".into()),
            source: Some("notion".into()),
            title: Some("Refund policy".into()),
        },
    )
    .await
    .unwrap();
    assert_eq!(ingested.source, "notion");
    ingest(
        &config,
        BrainIngestParams {
            path: None,
            text: Some("Onboarding takes a week.".into()),
            source: None,
            title: None,
        },
    )
    .await
    .unwrap();

    let docs = stored(&engine, MetaFilter::kinds([ItemKind::Document])).await;
    assert_eq!(docs.len(), 2);
    assert_eq!(
        jobs::snapshot(&config).await.pending.len(),
        2,
        "one belief build per source"
    );

    let counts = sources(&config).await.unwrap();
    assert_eq!(counts.root, "root");
    assert_eq!(counts.sources.len(), 2);
    assert_eq!(counts.unfiled, 0);

    let found = search(
        &config,
        BrainSearchParams {
            query: "refunds".into(),
            source: Some("notion".into()),
            limit: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(found.hits.len(), 1);

    let gone = forget(
        &config,
        BrainForgetParams {
            source: "notion".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(gone.forgotten, 1);
    assert_eq!(
        stored(&engine, MetaFilter::kinds([ItemKind::Document]))
            .await
            .len(),
        1
    );
}

#[tokio::test]
async fn ingest_refuses_bad_input() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    bind_reference(&config);
    for params in [
        BrainIngestParams {
            path: None,
            text: None,
            source: None,
            title: None,
        },
        BrainIngestParams {
            path: Some("/nope/missing.md".into()),
            text: None,
            source: None,
            title: None,
        },
        BrainIngestParams {
            path: None,
            text: Some("x".into()),
            source: Some(" ".into()),
            title: None,
        },
    ] {
        assert!(matches!(
            ingest(&config, params).await,
            Err(MemoryError::InvalidRequest(_))
        ));
    }
    assert!(search(
        &config,
        BrainSearchParams {
            query: " ".into(),
            source: None,
            limit: None
        }
    )
    .await
    .is_err());
}

#[tokio::test]
async fn a_file_is_converted_and_filed_under_files() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    let engine = bind_reference(&config);
    let path = tmp.path().join("guide.md");
    std::fs::write(&path, "# Guide\n\nAlways tag releases.").unwrap();
    let ingested = ingest(
        &config,
        BrainIngestParams {
            path: Some(path.display().to_string()),
            text: None,
            source: None,
            title: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(ingested.source, "files");
    let docs = stored(&engine, MetaFilter::kinds([ItemKind::Document])).await;
    assert_eq!(docs[0].meta.namespace.to_string(), "source:files");
}

#[tokio::test]
async fn an_ingest_queues_a_belief_build_only_for_an_engine_that_waits_for_one() {
    use std::sync::Arc;
    use tinymemory_api::conformance::ReferenceEngine;
    use tinymemory_api::Consolidation;

    for (consolidation, queued) in [(Consolidation::OnDemand, 1), (Consolidation::Automatic, 0)] {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_in(&tmp);
        crate::memory::engine::install_test_engine(
            &config.workspace_dir,
            Arc::new(ReferenceEngine::new().with_consolidation(consolidation)),
        );
        ingest(
            &config,
            BrainIngestParams {
                path: None,
                text: Some("Refunds are issued within 14 days.".into()),
                source: Some("notion".into()),
                title: None,
            },
        )
        .await
        .unwrap();
        let pending = crate::memory::lifecycle::jobs::snapshot(&config)
            .await
            .pending
            .len();
        assert_eq!(pending, queued, "{consolidation:?}");
    }
}
