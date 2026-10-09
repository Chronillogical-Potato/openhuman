use super::*;
use tinyagents_session::transcript::TranscriptMessage;

fn meta() -> TranscriptMeta {
    TranscriptMeta {
        session_id: None,
        parent_session_id: None,
        agent_name: "alpha".into(),
        agent_id: Some("alpha".into()),
        agent_type: None,
        dispatcher: "native".into(),
        provider: None,
        model: None,
        created: "2026-10-01T00:00:00Z".into(),
        updated: "2026-10-01T00:00:00Z".into(),
        turn_count: 1,
        prefix_message_count: None,
        input_tokens: 0,
        output_tokens: 0,
        cached_input_tokens: 0,
        charged_amount_usd: 0.0,
        thread_id: Some("thread-1".into()),
        task_id: None,
    }
}

fn messages(session: &dyn TranscriptRead) -> Vec<String> {
    session
        .read_session()
        .expect("readable")
        .expect("written")
        .messages
        .iter()
        .map(|message| message.content.clone())
        .collect()
}

#[test]
fn new_sessions_are_written_under_the_agents_directory() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let files = AgentTranscriptFiles::new(workspace.path(), "alpha");
    let session = SessionRef::scoped("thread-1", "alpha");

    files
        .open_session(&session, meta())
        .expect("bind")
        .replace(&[TranscriptMessage::user("hello")])
        .expect("write");

    let own = workspace.path().join("agents/alpha/session_raw");
    assert!(std::fs::read_dir(&own).expect("own dir").next().is_some());
    assert!(!workspace
        .path()
        .join("session_raw")
        .join(format!("{}.jsonl", session_stem(&session)))
        .exists());
}

#[test]
fn a_shared_session_is_read_then_continued_without_touching_the_shared_file() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let session = SessionRef::scoped("thread-1", "alpha");
    FileTranscriptLocator::new(workspace.path())
        .open_session(&session, meta())
        .expect("bind shared")
        .replace(&[TranscriptMessage::user("before")])
        .expect("write shared");
    let shared_path = workspace
        .path()
        .join("session_raw")
        .join(format!("{}.jsonl", session_stem(&session)));
    let shared_bytes = std::fs::read(&shared_path).expect("shared file");

    let files = AgentTranscriptFiles::new(workspace.path(), "alpha");
    let found = files
        .read_session_transcript(&session)
        .expect("the shared session is found");
    assert_eq!(messages(found.as_ref()), ["before"]);

    files
        .open_session(&session, meta())
        .expect("bind own")
        .replace(&[
            TranscriptMessage::user("before"),
            TranscriptMessage::user("after"),
        ])
        .expect("continue");

    assert_eq!(
        std::fs::read(&shared_path).expect("shared file"),
        shared_bytes
    );
    let continued = files
        .read_session_transcript(&session)
        .expect("own session");
    assert_eq!(messages(continued.as_ref()), ["before", "after"]);
}

#[test]
fn another_agent_does_not_adopt_a_session_scoped_to_someone_else() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let alpha_session = SessionRef::scoped("thread-1", "alpha");
    AgentTranscriptFiles::new(workspace.path(), "alpha")
        .open_session(&alpha_session, meta())
        .expect("bind")
        .replace(&[TranscriptMessage::user("alpha only")])
        .expect("write");

    let beta = AgentTranscriptFiles::new(workspace.path(), "beta");
    assert!(beta
        .read_session_transcript(&SessionRef::scoped("thread-1", "beta"))
        .is_none());
    assert!(beta.read_session_transcript(&alpha_session).is_none());
}
