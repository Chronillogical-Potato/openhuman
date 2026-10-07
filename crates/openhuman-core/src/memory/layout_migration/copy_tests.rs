use std::sync::atomic::{AtomicUsize, Ordering};

use tinymemory_api::conformance::ReferenceEngine;
use tinymemory_api::{LearningKind, MemoryMeta, Namespace, SourceKind, SourceRef};
use tinymemory_tools::MemoryLayout;

use super::*;
use crate::memory::layout_migration::map::FlowPlacement;
use crate::memory::test_fixtures::RefusingEngine;

fn placement() -> Placement {
    Placement {
        layout: MemoryLayout::new(Namespace::ROOT).unwrap(),
        chat_node: "ws:main".parse().unwrap(),
        flows: FlowPlacement::WithRoot,
    }
}

/// A legacy tree of `learnings` root learnings and one PDF in `source:pdf`.
async fn legacy_tree(learnings: usize) -> Arc<ReferenceEngine> {
    let legacy = Arc::new(ReferenceEngine::new());
    for n in 0..learnings {
        legacy
            .store(StoreItem::learning(
                format!("fact {n}"),
                LearningKind::Fact,
                0.5,
                MemoryMeta::default(),
            ))
            .await
            .unwrap();
    }
    legacy
        .store(StoreItem::document(
            "handbook",
            MemoryMeta {
                namespace: "source:pdf".parse().unwrap(),
                source: SourceRef {
                    kind: SourceKind::File,
                    id: None,
                },
                ..MemoryMeta::default()
            },
        ))
        .await
        .unwrap();
    legacy
}

async fn count(engine: &dyn MemoryEngine) -> usize {
    let mut total = 0;
    let mut cursor = None;
    loop {
        let mut request = ListRequest::new(MetaFilter::default(), 100);
        request.cursor = cursor;
        let page = engine.list(request).await.unwrap();
        total += page.items.len();
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => return total,
        }
    }
}

#[tokio::test]
async fn an_empty_legacy_tree_has_nothing_to_move() {
    assert!(!legacy_present(&ReferenceEngine::new()).await.unwrap());
    assert!(legacy_present(legacy_tree(0).await.as_ref()).await.unwrap());
}

#[tokio::test]
async fn every_item_is_copied_placed_and_read_back() {
    let tmp = tempfile::tempdir().unwrap();
    let legacy = legacy_tree(120).await;
    let tree = Arc::new(ReferenceEngine::new());
    let engines = Engines {
        legacy: legacy.clone(),
        tree: tree.clone(),
    };
    let mut state = MigrationState::default();
    copy(tmp.path(), &engines, &placement(), &mut state, || async {
        false
    })
    .await
    .unwrap();
    assert_eq!(state.phase, Phase::Copied);
    assert_eq!(state.copied, 121);
    assert!(state.failures.is_empty(), "{:?}", state.failures);
    assert_eq!(count(tree.as_ref()).await, 121);
    assert_eq!(
        count(legacy.as_ref()).await,
        121,
        "the copy deletes nothing"
    );
    let files = tree
        .list(ListRequest::new(
            MetaFilter {
                kinds: vec![tinymemory_api::ItemKind::Document],
                ..MetaFilter::default()
            },
            10,
        ))
        .await
        .unwrap();
    assert_eq!(files.items[0].meta.namespace.to_string(), "source:files");
    assert_eq!(state::load(tmp.path()).unwrap(), state, "saved as it ended");
}

#[tokio::test]
async fn a_paused_copy_resumes_where_it_stopped_without_duplicates() {
    let tmp = tempfile::tempdir().unwrap();
    let legacy = legacy_tree(120).await;
    let tree = Arc::new(ReferenceEngine::new());
    let engines = Engines {
        legacy,
        tree: tree.clone(),
    };
    let pages = AtomicUsize::new(0);
    let mut state = MigrationState::default();
    copy(tmp.path(), &engines, &placement(), &mut state, || {
        let stop = pages.fetch_add(1, Ordering::SeqCst) >= 1;
        async move { stop }
    })
    .await
    .unwrap();
    assert_eq!(state.phase, Phase::Paused);
    assert_eq!(state.copied, PAGE as u64, "one page before the pause");
    assert!(state.cursor.is_some());

    // A crash here: resume from what was saved, not from memory.
    let mut resumed = state::load(tmp.path()).unwrap();
    copy(tmp.path(), &engines, &placement(), &mut resumed, || async {
        false
    })
    .await
    .unwrap();
    assert_eq!(resumed.phase, Phase::Copied);
    assert_eq!(resumed.copied, 121);
    assert_eq!(count(tree.as_ref()).await, 121, "no item stored twice");
}

#[tokio::test]
async fn a_repeated_page_is_a_replay() {
    let tmp = tempfile::tempdir().unwrap();
    let legacy = legacy_tree(3).await;
    let tree = Arc::new(ReferenceEngine::new());
    let engines = Engines {
        legacy,
        tree: tree.clone(),
    };
    let mut first = MigrationState::default();
    copy(tmp.path(), &engines, &placement(), &mut first, || async {
        false
    })
    .await
    .unwrap();
    let mut again = MigrationState::default();
    copy(tmp.path(), &engines, &placement(), &mut again, || async {
        false
    })
    .await
    .unwrap();
    assert_eq!(again.copied, 4);
    assert_eq!(again.replayed, 4);
    assert_eq!(count(tree.as_ref()).await, 4);
}

#[tokio::test]
async fn an_account_that_cannot_write_pauses_without_advancing() {
    let tmp = tempfile::tempdir().unwrap();
    let engines = Engines {
        legacy: legacy_tree(3).await,
        tree: Arc::new(RefusingEngine::out_of_credits()),
    };
    let mut state = MigrationState::default();
    copy(tmp.path(), &engines, &placement(), &mut state, || async {
        false
    })
    .await
    .unwrap();
    assert_eq!(state.phase, Phase::Paused);
    assert_eq!(state.cursor, None, "the page is sent again on resume");
    assert_eq!(state.copied, 0);
    assert!(state
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("credits"));
}

#[tokio::test]
async fn a_legacy_tree_that_cannot_be_read_pauses() {
    let tmp = tempfile::tempdir().unwrap();
    let engines = Engines {
        legacy: Arc::new(RefusingEngine::with(tinymemory_api::Error::Unavailable(
            "down".into(),
        ))),
        tree: Arc::new(ReferenceEngine::new()),
    };
    let mut state = MigrationState::default();
    copy(tmp.path(), &engines, &placement(), &mut state, || async {
        false
    })
    .await
    .unwrap();
    assert_eq!(state.phase, Phase::Paused);
}

#[tokio::test]
async fn old_brain_nodes_are_refiled_by_connector() {
    let tmp = tempfile::tempdir().unwrap();
    let legacy = Arc::new(ReferenceEngine::new());
    let doc = |text: &str, node: &str, kind: SourceKind, path: Option<&str>| {
        StoreItem::document(
            text,
            MemoryMeta {
                namespace: node.parse().unwrap(),
                source: SourceRef { kind, id: None },
                file_path: path.map(str::to_string),
                ..MemoryMeta::default()
            },
        )
    };
    for item in [
        doc("a pdf", "source:pdf", SourceKind::File, Some("a.pdf")),
        doc("a note", "source:markdown", SourceKind::File, Some("a.md")),
        doc("a link", "source:web", SourceKind::Link, None),
        doc(
            "an upload",
            "source:web",
            SourceKind::Link,
            Some("page.html"),
        ),
        doc(
            "a drive doc",
            "source:google_drive",
            SourceKind::Composio,
            None,
        ),
        doc("a notion page", "source:notion", SourceKind::Composio, None),
    ] {
        legacy.store(item).await.unwrap();
    }
    let tree = Arc::new(ReferenceEngine::new());
    let engines = Engines {
        legacy,
        tree: tree.clone(),
    };
    let mut state = MigrationState::default();
    copy(tmp.path(), &engines, &placement(), &mut state, || async {
        false
    })
    .await
    .unwrap();
    assert_eq!(state.phase, Phase::Copied);
    let mut placed: Vec<(String, String)> = tree
        .list(ListRequest::new(MetaFilter::default(), 50))
        .await
        .unwrap()
        .items
        .into_iter()
        .map(|hit| (hit.text, hit.meta.namespace.to_string()))
        .collect();
    placed.sort();
    let expect = |text: &str, node: &str| (text.to_string(), node.to_string());
    assert_eq!(
        placed,
        vec![
            expect("a drive doc", "source:googledrive"),
            expect("a link", "source:web"),
            expect("a note", "source:files"),
            expect("a notion page", "source:notion"),
            expect("a pdf", "source:files"),
            expect("an upload", "source:files"),
        ]
    );
}
