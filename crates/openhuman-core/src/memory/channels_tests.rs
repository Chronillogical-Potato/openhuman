use super::*;

#[test]
fn records_threads_per_channel_case_insensitively() {
    let tmp = tempfile::tempdir().unwrap();
    record(tmp.path(), "Telegram", "t-1");
    record(tmp.path(), "telegram", "t-1");
    record(tmp.path(), "telegram", "t-2");
    record(tmp.path(), "", "t-3");
    record(tmp.path(), "web", " ");
    assert_eq!(threads_of(tmp.path(), "TELEGRAM"), ["t-1", "t-2"]);
    assert!(threads_of(tmp.path(), "web").is_empty());
}

#[tokio::test]
async fn a_channel_forgotten_while_signed_out_queues_each_thread_it_owned() {
    use crate::memory::deletion::{pending, PendingDeletion};
    use crate::memory::test_fixtures::config_in;

    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    record(&config.workspace_dir, "telegram", "t-1");
    record(&config.workspace_dir, "telegram", "t-2");

    assert_eq!(forget_channel(&config, "telegram").await.unwrap(), 0);
    assert_eq!(
        pending(&config.workspace_dir),
        vec![
            PendingDeletion::Thread {
                thread_id: "t-1".into()
            },
            PendingDeletion::Thread {
                thread_id: "t-2".into()
            },
        ]
    );
    // The channel's record goes now, so a thread it brings after a
    // reconnect is never caught by the queued deletion.
    assert!(threads_of(&config.workspace_dir, "telegram").is_empty());
}

#[tokio::test]
async fn a_channels_conversations_are_forgotten_and_others_kept() {
    use crate::memory::test_fixtures::{bind_reference, config_in, stored};
    use tinymemory_api::{MemoryEngine, MemoryMeta, Role, StoreItem, Turn};

    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    let engine = bind_reference(&config);
    for thread in ["t-chan", "t-web"] {
        engine
            .store(StoreItem::Conversation {
                turns: vec![Turn::new(Role::User, thread.to_string())],
                meta: MemoryMeta {
                    thread_id: Some(thread.to_string()),
                    ..MemoryMeta::default()
                },
            })
            .await
            .unwrap();
    }
    record(&config.workspace_dir, "telegram", "t-chan");

    assert_eq!(forget_channel(&config, "telegram").await.unwrap(), 1);
    let left = stored(&engine, MetaFilter::default()).await;
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].meta.thread_id.as_deref(), Some("t-web"));
    assert!(crate::memory::deletion::pending(&config.workspace_dir).is_empty());
}
