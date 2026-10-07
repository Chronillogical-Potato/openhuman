use super::*;
use crate::config::schema::MemorySourceKind;
use crate::memory::test_fixtures::{bind_reference, config_in, stored};
use tinymemory_api::{ItemKind, MetaFilter};
use tinymemory_tools::MemoryLayout;

fn record(id: &str, title: &str, content: &str) -> ConnectorRecord {
    ConnectorRecord {
        item_id: id.to_string(),
        title: title.to_string(),
        content: content.to_string(),
        ..ConnectorRecord::default()
    }
}

#[test]
fn record_item_builds_a_tagged_document() {
    let mut rec = record("m-1", "  Quarterly plan ", "ship memory v2");
    rec.mime = Some("text/plain".into());
    rec.url = Some("https://mail.example/m-1".into());
    rec.updated_at_ms = Some(1_700_000_000_000);
    rec.tags = vec!["inbox".into(), " ".into(), "gmail".into(), "inbox".into()];

    let item = record_item("GMail", "conn-7", "src-g", &rec).expect("an item");
    let StoreItem::Document {
        title,
        body,
        mime,
        meta,
    } = item
    else {
        panic!("expected a document");
    };
    assert_eq!(title.as_deref(), Some("Quarterly plan"));
    assert!(matches!(body, DocumentBody::Text(ref t) if t == "ship memory v2"));
    assert_eq!(mime.as_deref(), Some("text/plain"));
    assert_eq!(meta.url.as_deref(), Some("https://mail.example/m-1"));
    assert_eq!(meta.source.kind, SourceKind::Composio);
    assert_eq!(meta.source.id.as_deref(), Some("src-g"));
    assert_eq!(
        meta.tags,
        vec![
            "gmail".to_string(),
            "connection:conn-7".to_string(),
            "inbox".to_string()
        ],
        "toolkit and connection first, blanks and duplicates dropped"
    );
    assert_eq!(
        meta.observed_at.map(|t| t.timestamp_millis()),
        Some(1_700_000_000_000)
    );
}

#[test]
fn record_item_skips_empty_content_and_blank_titles() {
    assert!(record_item("gmail", "c", "s", &record("1", "t", "   ")).is_none());
    let StoreItem::Document { title, .. } =
        record_item("gmail", "c", "s", &record("1", "  ", "body")).unwrap()
    else {
        panic!("expected a document");
    };
    assert!(title.is_none());
}

#[test]
fn connection_tag_is_namespaced() {
    assert_eq!(connection_tag("abc"), "connection:abc");
}

#[tokio::test]
async fn store_records_stores_the_non_empty_ones() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    let engine = bind_reference(&config);
    let bound = crate::memory::engine::resolve(&config).engine().unwrap();
    let records = vec![
        record("1", "One", "first record"),
        record("2", "Empty", " "),
        record("3", "Three", "third record"),
    ];
    let stored_count = store_records(
        &config,
        &bound,
        "notion",
        "conn-1",
        "src-n",
        &MemoryLayout::default(),
        &records,
    )
    .await
    .unwrap();
    assert_eq!(stored_count, 2);
    let docs = stored(
        &engine,
        MetaFilter {
            kinds: vec![ItemKind::Document],
            ..MetaFilter::default()
        },
    )
    .await;
    assert_eq!(docs.len(), 2);
    assert!(docs
        .iter()
        .all(|d| d.meta.source.kind == SourceKind::Composio));
    assert!(
        docs.iter()
            .all(|d| d.meta.namespace.to_string() == "source:notion"),
        "filed under the toolkit's brain source"
    );
    assert_eq!(
        crate::memory::lifecycle::jobs::snapshot(&config)
            .await
            .pending
            .len(),
        1
    );
}

#[tokio::test]
async fn store_records_with_nothing_to_store_is_zero() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    bind_reference(&config);
    let bound = crate::memory::engine::resolve(&config).engine().unwrap();
    assert_eq!(
        store_records(
            &config,
            &bound,
            "notion",
            "c",
            "s",
            &MemoryLayout::default(),
            &[]
        )
        .await
        .unwrap(),
        0
    );
}

#[test]
fn source_id_for_toolkit_prefers_the_configured_source() {
    let tmp = tempfile::tempdir().unwrap();
    let mut config = config_in(&tmp);
    assert_eq!(source_id_for_toolkit(&config, "Gmail"), "composio:gmail");
    config
        .memory
        .sources
        .push(crate::config::schema::MemorySourceConfig {
            id: "src-gmail".into(),
            kind: MemorySourceKind::Composio,
            target: "gmail".into(),
            label: "Gmail".into(),
            schedule_mins: None,
            namespace: None,
        });
    config
        .memory
        .sources
        .push(crate::config::schema::MemorySourceConfig {
            id: "src-folder".into(),
            kind: MemorySourceKind::Folder,
            target: "notion".into(),
            label: "Folder named like a toolkit".into(),
            schedule_mins: None,
            namespace: None,
        });
    assert_eq!(source_id_for_toolkit(&config, "GMAIL"), "src-gmail");
    assert_eq!(source_id_for_toolkit(&config, "notion"), "composio:notion");

    // A source saved before targets were canonicalized still matches the
    // slug Composio reports for its connections.
    config
        .memory
        .sources
        .push(crate::config::schema::MemorySourceConfig {
            id: "src-drive".into(),
            kind: MemorySourceKind::Composio,
            target: "google_drive".into(),
            label: "Drive".into(),
            schedule_mins: None,
            namespace: None,
        });
    assert_eq!(source_id_for_toolkit(&config, "googledrive"), "src-drive");
    assert_eq!(source_id_for_toolkit(&config, "google_drive"), "src-drive");
}

#[tokio::test]
async fn forget_connection_removes_only_that_connections_items() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    let engine = bind_reference(&config);
    let bound = crate::memory::engine::resolve(&config).engine().unwrap();
    store_records(
        &config,
        &bound,
        "gmail",
        "conn-a",
        "src",
        &MemoryLayout::default(),
        &[record("1", "A", "from a")],
    )
    .await
    .unwrap();
    store_records(
        &config,
        &bound,
        "gmail",
        "conn-b",
        "src",
        &MemoryLayout::default(),
        &[record("2", "B", "from b")],
    )
    .await
    .unwrap();
    assert_eq!(
        forget_connection(&config, "conn-a", Some("gmail"))
            .await
            .unwrap(),
        1
    );
    let left = stored(&engine, MetaFilter::default()).await;
    assert_eq!(left.len(), 1);
    assert!(left[0].meta.tags.contains(&"connection:conn-b".to_string()));
}

#[tokio::test]
async fn forget_connection_reads_every_root_its_items_were_filed_under() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    let engine = bind_reference(&config);
    let bound = crate::memory::engine::resolve(&config).engine().unwrap();
    // One connection's items under the current root and under one it used
    // before (a source namespace since changed), plus another connection.
    for (connection, layout) in [
        ("conn-c", MemoryLayout::default()),
        (
            "conn-c",
            MemoryLayout::new("team:old".parse().unwrap()).unwrap(),
        ),
        ("conn-d", MemoryLayout::default()),
    ] {
        store_records(
            &config,
            &bound,
            "gmail",
            connection,
            "src",
            &layout,
            &[record(
                &format!("{connection}-{}", layout.root()),
                "m",
                "mail",
            )],
        )
        .await
        .unwrap();
    }
    assert_eq!(
        super::super::roots::of(&config.workspace_dir, "conn-c")
            .unwrap()
            .len(),
        2
    );

    assert_eq!(
        forget_connection(&config, "conn-c", Some("gmail"))
            .await
            .unwrap(),
        2,
        "both roots' items, though the current one alone found some"
    );
    let left = stored(&engine, MetaFilter::default()).await;
    assert_eq!(left.len(), 1);
    assert!(left[0].meta.tags.contains(&"connection:conn-d".to_string()));
    assert!(super::super::roots::of(&config.workspace_dir, "conn-c")
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn an_edited_record_replaces_its_previous_version() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    let engine = bind_reference(&config);
    let bound = crate::memory::engine::resolve(&config).engine().unwrap();
    let sync = |records: Vec<ConnectorRecord>| {
        let (config, bound) = (&config, &bound);
        async move {
            store_records(
                config,
                bound,
                "notion",
                "conn-a",
                "src",
                &MemoryLayout::default(),
                &records,
            )
            .await
            .unwrap()
        }
    };
    let texts = || async {
        let mut texts: Vec<String> = stored(&engine, MetaFilter::default())
            .await
            .into_iter()
            .map(|hit| hit.text)
            .collect();
        texts.sort();
        texts
    };

    sync(vec![
        record("p1", "Plan", "v1"),
        record("p2", "Notes", "kept"),
    ])
    .await;
    // The same records again: nothing is stale.
    sync(vec![
        record("p1", "Plan", "v1"),
        record("p2", "Notes", "kept"),
    ])
    .await;
    assert_eq!(texts().await, ["# Notes\n\nkept", "# Plan\n\nv1"]);

    // p1 edited upstream: its old version goes, p2 stays.
    assert_eq!(sync(vec![record("p1", "Plan", "v2")]).await, 1);
    assert_eq!(texts().await, ["# Notes\n\nkept", "# Plan\n\nv2"]);

    // p2 comes back empty upstream: its stored version goes.
    sync(vec![record("p2", "Notes", "  ")]).await;
    assert_eq!(texts().await, ["# Plan\n\nv2"]);

    // Disconnecting drops the record ids with the items.
    forget_connection(&config, "conn-a", Some("notion"))
        .await
        .unwrap();
    assert!(super::super::versions::begin(
        &config.workspace_dir,
        &[(super::super::versions::key("conn-a", "p1"), "other".into())],
        &[]
    )
    .is_empty());
}

#[tokio::test]
async fn forget_connection_with_memory_off_forgets_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    assert_eq!(
        forget_connection(&config, "conn-a", Some("gmail"))
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn forget_connection_reads_only_its_toolkits_source() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    let engine = bind_reference(&config);
    let bound = crate::memory::engine::resolve(&config).engine().unwrap();
    // The same connection tag in another toolkit's source: an item the
    // scoped forget must not reach.
    for toolkit in ["gmail", "notion"] {
        store_records(
            &config,
            &bound,
            toolkit,
            "conn-a",
            "src",
            &MemoryLayout::default(),
            &[record(toolkit, toolkit, &format!("from {toolkit}"))],
        )
        .await
        .unwrap();
    }
    assert_eq!(
        forget_connection(&config, "conn-a", Some("gmail"))
            .await
            .unwrap(),
        1
    );
    let left = stored(&engine, MetaFilter::default()).await;
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].meta.namespace.to_string(), "source:notion");

    // Items filed under a root no longer configured (a removed source's own
    // namespace) are outside the toolkit's source: the scoped forget finds
    // none and falls back to the whole tree.
    let elsewhere = MemoryLayout::new("team:old".parse().unwrap()).unwrap();
    store_records(
        &config,
        &bound,
        "gmail",
        "conn-b",
        "src",
        &elsewhere,
        &[record("b", "b", "from an old root")],
    )
    .await
    .unwrap();
    assert_eq!(
        forget_connection(&config, "conn-b", Some("gmail"))
            .await
            .unwrap(),
        1
    );

    // An unknown toolkit falls back to the whole tree.
    assert_eq!(forget_connection(&config, "conn-a", None).await.unwrap(), 1);
    assert!(stored(&engine, MetaFilter::default()).await.is_empty());
}

#[tokio::test]
async fn sync_toolkit_without_a_connector_is_an_error_not_a_panic() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    bind_reference(&config);
    let bound = crate::memory::engine::resolve(&config).engine().unwrap();
    let source = crate::config::schema::MemorySourceConfig {
        id: "src-gmail".into(),
        kind: MemorySourceKind::Composio,
        target: "gmail".into(),
        label: "Gmail".into(),
        schedule_mins: None,
        namespace: None,
    };
    assert!(sync_toolkit(&config, &bound, &source).await.is_err());
}

#[tokio::test]
async fn a_disconnect_waits_for_a_store_in_progress() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    bind_reference(&config);
    // A store of this connection's records is running.
    let held = STORE.lock().await;
    let forget = {
        let config = config.clone();
        tokio::spawn(async move { forget_connection(&config, "conn-w", Some("gmail")).await })
    };
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(!forget.is_finished(), "the disconnect waits for the store");
    drop(held);
    assert_eq!(forget.await.unwrap().unwrap(), 0);
}

#[tokio::test]
async fn records_read_before_a_disconnect_are_not_stored_after_it() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    let engine = bind_reference(&config);
    let bound = crate::memory::engine::resolve(&config).engine().unwrap();
    forget_connection(&config, "conn-gone", Some("gmail"))
        .await
        .unwrap();
    assert!(is_disconnected(&config, "conn-gone"));
    // A pass that read its records before the disconnect stores nothing.
    let stored_now = store_records(
        &config,
        &bound,
        "gmail",
        "conn-gone",
        "src",
        &MemoryLayout::default(),
        &[record("late", "Late", "read before the disconnect")],
    )
    .await
    .unwrap();
    assert_eq!(stored_now, 0);
    assert!(stored(&engine, MetaFilter::default()).await.is_empty());
    // Another connection, or the same id in another workspace, is unaffected.
    assert!(!is_disconnected(&config, "conn-other"));
}
