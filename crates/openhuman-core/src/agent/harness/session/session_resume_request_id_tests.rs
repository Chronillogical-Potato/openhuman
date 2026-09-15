use super::*;

/// #6282: rows seeded from the conversation log belong to earlier turns and
/// carry no request id, so persisting them together with the next turn must
/// not stamp them with that turn's request.
#[test]
fn prose_seeded_rows_are_not_restamped_with_the_resuming_request() {
    use crate::agent::harness::session::transcript::{
        append_transcript_turn, read_transcript_display, DisplayRecord,
    };
    use crate::agent::messages::{ChatMessage, ConversationMessage};

    let mut agent = build_minimal_agent_with_definition_name(Some("orchestrator"));
    agent
        .seed_resume_from_messages(
            vec![
                ("user".to_string(), "install it".to_string()),
                ("agent".to_string(), "Something went wrong.".to_string()),
            ],
            "what happened?",
        )
        .expect("seed");
    // The resuming turn: its own message, then the seeded prefix absorbed ahead
    // of it and rendered for persistence, exactly as the turn driver does.
    agent.history = vec![ConversationMessage::Chat(ChatMessage::user(
        "what happened?",
    ))];
    agent.absorb_resumed_transcript_prefix();
    let messages = agent.tool_dispatcher.to_provider_messages(&agent.history);

    let dir = tempfile::TempDir::new().expect("temp dir");
    let path = dir.path().join("seeded.jsonl");
    append_transcript_turn(
        &path,
        &[],
        &messages,
        &fake_transcript_meta("thr_seeded"),
        None,
        Some("req-now"),
    )
    .expect("persist seeded turn");

    let rows: Vec<(String, Option<String>)> = read_transcript_display(&path)
        .expect("display read")
        .records
        .into_iter()
        .filter_map(|record| match record {
            DisplayRecord::Message(m) => Some((m.message.content, m.request_id)),
            _ => None,
        })
        .collect();
    let ids: Vec<Option<&str>> = rows.iter().map(|(_, id)| id.as_deref()).collect();
    assert_eq!(
        ids,
        vec![None, None, None, Some("req-now")],
        "the seeded prefix (its system prompt included) keeps no request id; only \
         this turn's new message takes it: {rows:?}"
    );
}

/// #6282: a thread transcript written without request ids (a CLI or full-rewrite
/// transcript) resumed into a request-scoped turn must not have its replayed
/// rows restamped with the resuming request, which would group every earlier
/// message into the current turn.
#[test]
fn a_resumed_request_less_transcript_is_not_restamped_with_the_resuming_request() {
    use super::super::transcript::{self, read_transcript_display, DisplayRecord};
    use crate::agent::messages::{ChatMessage, ConversationMessage};

    let ws = tempfile::TempDir::new().expect("temp workspace");
    let wsp = ws.path().to_path_buf();
    let thread_id = "thr_request_less";
    let path = transcript::resolve_keyed_transcript_path(&wsp, "1700000000_orchestrator")
        .expect("resolve transcript path");
    transcript::write_transcript(
        &path,
        &[
            ChatMessage::system("stored prompt"),
            ChatMessage::user("first question"),
            ChatMessage::assistant("first answer"),
        ],
        &fake_transcript_meta(thread_id),
        None,
    )
    .expect("write transcript");

    let mem: Arc<dyn Memory> = crate::memory::test_support::noop_memory();
    let mut agent = Agent::builder()
        .chat_model(Arc::new(MockProvider {
            responses: Mutex::new(vec![]),
        }))
        .tools(vec![Box::new(MockTool)])
        .memory(mem)
        .tool_dispatcher(Box::new(NativeToolDispatcher))
        .workspace_dir(wsp.clone())
        .build()
        .expect("agent build should succeed");
    assert!(agent.seed_resume_from_thread_transcript(thread_id));

    // The resuming turn: its own message, then the replayed prefix absorbed
    // ahead of it, exactly as the turn driver does.
    agent.history = vec![ConversationMessage::Chat(ChatMessage::user(
        "second question",
    ))];
    agent.absorb_resumed_transcript_prefix();
    let messages = agent.tool_dispatcher.to_provider_messages(&agent.history);

    let out = wsp.join("resumed.jsonl");
    transcript::append_transcript_turn(
        &out,
        &[],
        &messages,
        &fake_transcript_meta(thread_id),
        None,
        Some("req-2"),
    )
    .expect("persist resumed turn");
    let rows: Vec<(String, Option<String>)> = read_transcript_display(&out)
        .expect("display read")
        .records
        .into_iter()
        .filter_map(|record| match record {
            DisplayRecord::Message(m) => Some((m.message.content, m.request_id)),
            _ => None,
        })
        .collect();
    let ids: Vec<Option<&str>> = rows.iter().map(|(_, id)| id.as_deref()).collect();
    assert_eq!(
        ids,
        vec![None, None, None, Some("req-2")],
        "replayed request-less rows keep no request id; only the resuming turn's row takes it: {rows:?}"
    );
}
