use super::*;

use std::time::Duration;

use tinymemory::{ItemKind, MetaFilter};

use crate::memory::conversations;
use crate::memory::error::{INVALID_REQUEST, MEMORY_OFF};
use crate::memory::test_fixtures::{bind_reference, config_in, stored};
use crate::threads::store::CreateConversationThread;

fn message(id: &str, sender: &str, content: &str, at: &str) -> ConversationMessage {
    ConversationMessage {
        id: id.to_string(),
        content: content.to_string(),
        message_type: "text".to_string(),
        extra_metadata: serde_json::Value::Null,
        sender: sender.to_string(),
        created_at: at.to_string(),
    }
}

async fn seed_thread(workspace: &Path, thread_id: &str, messages: &[(&str, &str)]) {
    threads::ensure_thread(
        workspace.to_path_buf(),
        CreateConversationThread {
            id: thread_id.to_string(),
            title: thread_id.to_string(),
            created_at: "2026-09-01T10:00:00Z".to_string(),
            parent_thread_id: None,
            labels: None,
            personality_id: None,
        },
    )
    .await
    .unwrap();
    for (index, (sender, content)) in messages.iter().enumerate() {
        threads::append_message(
            workspace.to_path_buf(),
            thread_id.to_string(),
            message(
                &format!("{thread_id}-{index}"),
                sender,
                content,
                &format!("2026-09-01T10:{:02}:00Z", index),
            ),
        )
        .await
        .unwrap();
    }
}

async fn wait_done(config: &Config) -> BackfillView {
    for _ in 0..200 {
        let view = status(config).await.unwrap();
        if view.state.phase != ImportPhase::Running {
            return view;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("the backfill did not finish");
}

#[test]
fn messages_become_turns_like_the_chat_shows_them() {
    let messages = [
        message("0", "agent", "Welcome!", "2026-09-01T09:59:00Z"),
        message("1", "user", "Plan a trip", "2026-09-01T10:00:00Z"),
        message("2", "agent", "Where to?", "2026-09-01T10:01:00Z"),
        message("3", "agent", "Also, when?", "2026-09-01T10:02:00Z"),
        message("4", "user", "   ", "2026-09-01T10:03:00Z"),
        message("5", "user", "Bali in October", "not a time"),
    ];
    let turns = turns_of("t", &messages);
    assert_eq!(turns.len(), 3);
    assert_eq!(turns[0].user, "");
    assert_eq!(turns[0].assistant, "Welcome!");
    assert_eq!(turns[1].user, "Plan a trip");
    assert_eq!(turns[1].assistant, "Where to?\n\nAlso, when?");
    assert_eq!(turns[1].at.to_rfc3339(), "2026-09-01T10:02:00+00:00");
    assert_eq!(turns[2].user, "Bali in October");
    assert!(turns[2].assistant.is_empty());
}

#[test]
fn the_range_leaves_live_turns_and_stored_turns_alone() {
    assert_eq!(pending_range(10, 0, 0), 0..10);
    assert_eq!(pending_range(10, 3, 0), 0..7, "the 3 newest are live's");
    assert_eq!(pending_range(10, 3, 4), 4..7);
    assert_eq!(pending_range(10, 3, 9), 7..7, "nothing left");
    assert_eq!(pending_range(2, 5, 0), 0..0, "live took more than exist");
}

#[test]
fn batches_cut_the_range_by_batch_turns() {
    let turns = turns_of(
        "t",
        &(0..5)
            .map(|i| {
                message(
                    &i.to_string(),
                    "user",
                    &format!("q{i}"),
                    "2026-09-01T10:00:00Z",
                )
            })
            .collect::<Vec<_>>(),
    );
    let cut = batches("t", &turns, 1..5, 3);
    assert_eq!(cut.len(), 2);
    assert_eq!((cut[0].first, cut[0].last()), (1, 3));
    assert_eq!((cut[1].first, cut[1].last()), (4, 4));
    assert!(batches("t", &turns, 2..2, 3).is_empty());
    assert_eq!(
        batches("t", &turns, 0..5, 0).len(),
        5,
        "zero means one per item"
    );
}

#[tokio::test]
async fn refuses_without_consent_and_with_memory_off() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    let error = start(&config, BackfillStartParams { consent: false })
        .await
        .unwrap_err();
    assert_eq!(error.code(), INVALID_REQUEST);
    let error = start(&config, BackfillStartParams { consent: true })
        .await
        .unwrap_err();
    assert_eq!(error.code(), MEMORY_OFF);
}

#[tokio::test]
async fn stores_past_chats_once_and_skips_what_live_ingestion_took() {
    let tmp = tempfile::tempdir().unwrap();
    let mut config = config_in(&tmp);
    config.memory.conversations.batch_turns = 2;
    let engine = bind_reference(&config);
    let workspace = config.workspace_dir.clone();

    seed_thread(
        &workspace,
        "old-thread",
        &[
            ("user", "What is ownership?"),
            ("agent", "Values have one owner."),
            ("user", "And borrowing?"),
            ("agent", "References without ownership."),
            ("user", "Thanks"),
            ("agent", "Any time."),
        ],
    )
    .await;
    seed_thread(
        &workspace,
        "live-thread",
        &[
            ("user", "Before ingestion"),
            ("agent", "An old answer."),
            ("user", "After ingestion"),
            ("agent", "A live answer."),
        ],
    )
    .await;
    // Live ingestion took live-thread's last turn.
    conversations::record_turn(
        &config,
        conversations::buffer::CommittedTurn {
            thread_id: "live-thread".into(),
            agent_id: None,
            namespace: tinymemory::Namespace::ROOT,
            workspace: None,
            channel: None,
            user: "After ingestion".into(),
            assistant: "A live answer.".into(),
            tool_calls: Vec::new(),
            at: Utc::now(),
        },
    )
    .await;

    let before = status(&config).await.unwrap();
    assert_eq!(before.state.phase, ImportPhase::Idle);
    assert_eq!((before.pending_threads, before.pending_turns), (2, 4));

    let started = start(&config, BackfillStartParams { consent: true })
        .await
        .unwrap();
    assert_eq!(started.state.threads_total, 2);
    let done = wait_done(&config).await;
    assert_eq!(done.state.phase, ImportPhase::Done, "{:?}", done.state);
    assert_eq!(done.state.turns_stored, 4);
    assert_eq!(
        done.state.items_stored, 3,
        "old-thread in two batches, live-thread in one"
    );
    assert!(done.state.finished_at.is_some());
    assert_eq!((done.pending_threads, done.pending_turns), (0, 0));

    let filter = MetaFilter {
        kinds: vec![ItemKind::Conversation],
        tags_any: vec![BACKFILL_TAG.to_string()],
        ..MetaFilter::default()
    };
    let items = stored(&engine, filter.clone()).await;
    assert_eq!(items.len(), 3);
    let live: Vec<_> = items
        .iter()
        .filter(|hit| hit.meta.thread_id.as_deref() == Some("live-thread"))
        .collect();
    assert_eq!(live.len(), 1);
    assert!(live[0].text.contains("Before ingestion"));
    assert!(
        !live[0].text.contains("After ingestion"),
        "live's turn is not re-stored"
    );

    // A second run has nothing to send.
    start(&config, BackfillStartParams { consent: true })
        .await
        .unwrap();
    let again = wait_done(&config).await;
    assert_eq!(again.state.items_stored, 0);
    assert_eq!(stored(&engine, filter).await.len(), 3);
}
