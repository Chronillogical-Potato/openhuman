use super::*;

use std::time::Duration;

use tinymemory_api::{ItemKind, MetaFilter};

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
            working_dir: None,
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
    let turns = turns_of(&messages);
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
fn the_range_stops_where_live_logging_began() {
    assert_eq!(pending_range(10, None, 0), 0..10);
    assert_eq!(pending_range(10, Some(7), 0), 0..7, "turns 7.. are live's");
    assert_eq!(pending_range(10, Some(7), 4), 4..7);
    assert_eq!(pending_range(10, Some(7), 9), 7..7, "nothing left");
    assert_eq!(pending_range(2, Some(5), 0), 0..2);
}

#[test]
fn a_turn_is_one_item_per_message_at_the_agents_node() {
    let config = Config::default();
    let identity = MemoryIdentity::agent(MAIN_AGENT).resolve(&config);
    let turn = PastTurn {
        user: "q".into(),
        assistant: "a".into(),
        at: Utc::now(),
    };
    let items = turn_items(&identity, "t", 3, &turn);
    assert_eq!(items.len(), 2);
    let meta = items[1].meta();
    assert_eq!(meta.turns.map(|range| range.first), Some(7));
    assert_eq!(meta.namespace.to_string(), "agent:orchestrator");
    assert!(meta.tags.iter().any(|tag| tag == BACKFILL_TAG));
    let reply_only = PastTurn {
        user: String::new(),
        ..turn
    };
    assert_eq!(turn_items(&identity, "t", 0, &reply_only).len(), 1);
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
async fn stores_past_chats_once_and_skips_what_live_logging_took() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
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
        ],
    )
    .await;
    seed_thread(
        &workspace,
        "live-thread",
        &[
            ("user", "Before logging"),
            ("agent", "An old answer."),
            ("user", "After logging"),
            ("agent", "A live answer."),
        ],
    )
    .await;
    // The lifecycle logged live-thread's second turn (user index 2).
    let identity = MemoryIdentity::agent(MAIN_AGENT).resolve(&config);
    crate::memory::lifecycle::hooks::pre_turn(
        &config,
        &identity,
        crate::memory::lifecycle::hooks::PreTurnInput {
            thread_id: "live-thread".into(),
            turn_index: 2,
            user_text: "After logging".into(),
            in_prompt_from: 0,
            at: Utc::now(),
            resumed_after_compaction: false,
            observed_actor: None,
        },
    )
    .await;

    let before = status(&config).await.unwrap();
    assert_eq!(before.state.phase, ImportPhase::Idle);
    assert_eq!((before.pending_threads, before.pending_turns), (2, 3));

    let started = start(&config, BackfillStartParams { consent: true })
        .await
        .unwrap();
    assert_eq!(started.state.threads_total, 2);
    let done = wait_done(&config).await;
    assert_eq!(done.state.phase, ImportPhase::Done, "{:?}", done.state);
    assert_eq!(done.state.turns_stored, 3);
    assert_eq!(done.state.items_stored, 6);
    assert!(done.state.finished_at.is_some());
    assert_eq!((done.pending_threads, done.pending_turns), (0, 0));

    let filter = MetaFilter {
        kinds: vec![ItemKind::Conversation],
        tags_any: vec![BACKFILL_TAG.to_string()],
        ..MetaFilter::default()
    };
    let items = stored(&engine, filter.clone()).await;
    assert_eq!(items.len(), 6);
    assert!(
        !items.iter().any(|hit| hit.text.contains("After logging")),
        "live's turn is not re-stored"
    );

    // A second run has nothing to send.
    start(&config, BackfillStartParams { consent: true })
        .await
        .unwrap();
    let again = wait_done(&config).await;
    assert_eq!(again.state.items_stored, 0);
    assert_eq!(stored(&engine, filter).await.len(), 6);
}
