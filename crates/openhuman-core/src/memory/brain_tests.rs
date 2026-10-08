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
fn synced_items_are_filed_under_files() {
    for kind in MemorySourceKind::ALL {
        assert_eq!(brain_source(kind), files_source(), "{kind:?}");
    }
    assert_eq!(files_source().to_string(), "files");
}

#[test]
fn a_github_document_names_its_repository() {
    let with = |repo: Option<&str>, url: Option<&str>| {
        let mut item = document(None, None);
        item.meta_mut().repo = repo.map(str::to_string);
        item.meta_mut().url = url.map(str::to_string);
        github_collection(&item)
    };
    assert_eq!(with(Some("Acme/API"), None).as_deref(), Some("acme--api"));
    assert_eq!(
        with(Some("https://github.com/acme/api.git"), None).as_deref(),
        Some("acme--api")
    );
    // A Composio issue carries only its URL.
    assert_eq!(
        with(
            None,
            Some("https://github.com/tinyhumansai/openhuman/issues/12")
        )
        .as_deref(),
        Some("tinyhumansai--openhuman")
    );
    assert_eq!(with(None, Some("https://example.com/a/b")), None);
    assert_eq!(with(Some("acme"), None), None);
    assert_eq!(with(None, None), None);
    // A query or fragment is not part of the repository.
    assert_eq!(
        with(None, Some("https://github.com/acme/api?tab=readme#top")).as_deref(),
        Some("acme--api")
    );
    // Hyphens on either side never make two repositories one collection.
    assert_ne!(
        with(Some("foo-bar/repo"), None),
        with(Some("foo/bar-repo"), None)
    );
}

#[tokio::test]
async fn a_sources_collections_count_and_forget_with_it() {
    use tinymemory_api::MemoryEngine as _;
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    let engine = bind_reference(&config);
    let layout = MemoryLayout::default();
    for (node, text) in [
        (layout.brain(&BrainSource::Github).unwrap(), "org notes"),
        (
            layout
                .brain_collection(&BrainSource::Github, "acme-api")
                .unwrap(),
            "an issue",
        ),
        (layout.brain(&BrainSource::Notion).unwrap(), "a page"),
    ] {
        let meta = MemoryMeta {
            namespace: node,
            ..MemoryMeta::default()
        };
        engine.store(StoreItem::document(text, meta)).await.unwrap();
    }
    let view = sources(&config).await.unwrap();
    let counts: Vec<(&str, u64)> = view
        .sources
        .iter()
        .map(|s| (s.source.as_str(), s.documents))
        .collect();
    assert_eq!(counts, [("github", 2), ("notion", 1)]);
    assert_eq!(view.unfiled, 0);

    let gone = forget(
        &config,
        BrainForgetParams {
            source: "github".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(gone.forgotten, 2, "the source and its collection");
}

#[test]
fn filing_moves_an_item_to_its_source_node_without_an_agent() {
    let team = MemoryLayout::new("team:acme".parse().unwrap()).unwrap();
    let filed = file_into(team.brain(&BrainSource::Pdf).unwrap(), document(None, None));
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

#[test]
fn legacy_nodes_map_to_their_connector() {
    use tinymemory_api::{SourceKind, SourceRef};
    let item = |kind: SourceKind, path: Option<&str>| {
        let mut item = document(None, path);
        item.meta_mut().source = SourceRef { kind, id: None };
        item
    };
    let link = item(SourceKind::Link, None);
    for old in [
        "pdf", "markdown", "md", "docx", "xlsx", "pptx", "code", "other", "files",
    ] {
        assert_eq!(legacy_brain_node(old, &link), files_source(), "{old}");
    }
    // `web` held links and feeds, and HTML files that were filed beside them.
    assert_eq!(legacy_brain_node("web", &link), BrainSource::Web);
    assert_eq!(
        legacy_brain_node("web", &item(SourceKind::Rss, None)),
        BrainSource::Web
    );
    assert_eq!(
        legacy_brain_node("web", &item(SourceKind::Folder, None)),
        files_source()
    );
    assert_eq!(
        legacy_brain_node("web", &item(SourceKind::File, None)),
        files_source()
    );
    // An upload carried the `Link` kind `web` implies, but kept its path.
    assert_eq!(
        legacy_brain_node("web", &item(SourceKind::Link, Some("/u/pricing.html"))),
        files_source()
    );
    // Connectors keep their node, under the slug Composio uses.
    assert_eq!(legacy_brain_node("notion", &link), BrainSource::Notion);
    assert_eq!(legacy_brain_node("github", &link), BrainSource::Github);
    assert_eq!(
        legacy_brain_node("gmail", &link),
        BrainSource::Other("gmail".into())
    );
    assert_eq!(
        legacy_brain_node("google_drive", &link),
        BrainSource::Other("googledrive".into())
    );
}
