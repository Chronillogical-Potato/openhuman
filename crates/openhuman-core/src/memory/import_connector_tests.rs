//! The import leaves out what v1 synced from Composio and the connectors.

use super::tests::{chunk_only_workspace, legacy_workspace, memory_doc, wait_until_settled};
use super::*;
use crate::memory::test_fixtures::{bind_reference, config_in, stored};
use rusqlite::Connection;
use tinymemory_api::MetaFilter;

#[tokio::test]
async fn connector_syncs_are_not_imported() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    legacy_workspace(&config.workspace_dir);
    // Composio leftovers: a `skill-gmail` doc, a Gmail identity facet, and a
    // chunk store with an email and a Slack source beside a memory-source
    // folder file that must still come across.
    let conn = Connection::open(config.workspace_dir.join("memory").join("memory.db")).unwrap();
    memory_doc(&conn, "d9", "skill-gmail", "Inbox", "Mail from Ann");
    conn.execute(
        "INSERT INTO user_profile (facet_id, facet_type, key, value, confidence, first_seen_at, last_seen_at)
         VALUES ('skill-gmail-c1-name', 'identity', 'skill:gmail:name', 'Ann', 0.9, 1.0, 1.0)",
        [],
    )
    .unwrap();
    drop(conn);
    chunk_only_workspace(&config.workspace_dir);
    let conn =
        Connection::open(config.workspace_dir.join("memory_tree").join("chunks.db")).unwrap();
    conn.execute_batch(
        "INSERT INTO mem_tree_chunks VALUES
           ('k4', 'email', 'thread-1', NULL, NULL, 'me', 4, 4, 4, '[]', 'invoice attached', 3, 0, 4),
           ('k5', 'chat', 'slack:conn1', NULL, NULL, 'me', 5, 5, 5, '[]', 'standup at 10', 3, 0, 5),
           ('k6', 'document', 'mem_src:folder', NULL, NULL, 'me', 6, 6, 6, '[]', 'folder note', 3, 0, 6);",
    )
    .unwrap();
    drop(conn);

    // Documents: 2 notes + 2 chunk sources (d1, mem_src); the skill-gmail doc
    // and the email/slack sources are absent. Learnings: doc + 1 facet.
    let counts = scan(&config).await.unwrap().counts.expect("counts");
    assert_eq!(
        (counts.documents, counts.conversations, counts.learnings),
        (5, 1, 2),
        "scan counts exclude connector syncs"
    );

    let engine = bind_reference(&config);
    start(&config, true).await.unwrap();
    let done = wait_until_settled(&config).await;
    assert_eq!(
        (done.phase, done.imported, done.total),
        (ImportPhase::Done, 8, 8)
    );
    let items = stored(&engine, MetaFilter::default()).await;
    assert_eq!(items.len(), 8);
    let ids: Vec<String> = items
        .iter()
        .filter_map(|hit| hit.meta.source.id.clone())
        .collect();
    assert_eq!(ids.len(), 8, "every item carries its legacy id");
    for skipped in [
        "memory_docs:d9",
        "user_profile:skill-gmail-c1-name",
        "mem_tree_chunks:email:thread-1",
        "mem_tree_chunks:chat:slack:conn1",
    ] {
        assert!(
            !ids.iter().any(|id| id == skipped),
            "{skipped} imported: {ids:?}"
        );
    }
    assert!(ids
        .iter()
        .any(|id| id == "mem_tree_chunks:document:mem_src:folder"));
}
