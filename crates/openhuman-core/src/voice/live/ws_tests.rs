use super::*;
use tinyagents_live::tinyliveagents::{AudioFormat, Error as LiveError, SessionInfo, ToolCall};

fn map(event: LiveEvent) -> Vec<Outbound> {
    map_event(LiveAgentEvent::Live(event), "sess", "sarvam", "thread")
}

#[test]
fn ready_uses_the_provider_session_id_or_ours() {
    let info = |id: Option<&str>| SessionInfo {
        provider: "sarvam".into(),
        session_id: id.map(str::to_string),
        model: None,
        input_format: AudioFormat::default(),
        output_format: AudioFormat::pcm16(24_000),
    };
    assert_eq!(
        map(LiveEvent::Ready(info(None))),
        vec![Outbound::Json(ServerFrame::Ready {
            session_id: Some("sess".into()),
            provider: "sarvam".into(),
            output_sample_rate: 24_000,
            thread_id: Some("thread".into())
        })]
    );
    let Outbound::Json(ServerFrame::Ready { session_id, .. }) =
        &map(LiveEvent::Ready(info(Some("p"))))[0]
    else {
        panic!("expected ready")
    };
    assert_eq!(session_id.as_deref(), Some("p"));
}

#[test]
fn finals_are_persisted_partials_are_not() {
    assert_eq!(
        map(LiveEvent::InputTranscript {
            text: "wha".into(),
            is_final: false
        }),
        vec![Outbound::Json(ServerFrame::Transcript {
            role: TranscriptRole::User,
            text: "wha".into(),
            is_final: false
        })]
    );
    let out = map(LiveEvent::OutputTranscript {
        text: "noon".into(),
        is_final: true,
    });
    assert_eq!(out.len(), 2);
    assert_eq!(
        out[1],
        Outbound::Persist(TranscriptRole::Agent, "noon".into())
    );
}

#[test]
fn maps_audio_turns_errors_and_closes() {
    assert_eq!(
        map(LiveEvent::Audio(Bytes::from_static(&[1, 2]))),
        vec![Outbound::Audio(Bytes::from_static(&[1, 2]))]
    );
    assert_eq!(
        map(LiveEvent::Interrupted),
        vec![Outbound::Json(ServerFrame::Interrupted)]
    );
    assert_eq!(
        map(LiveEvent::TurnComplete { usage: None }),
        vec![Outbound::Json(ServerFrame::TurnComplete)]
    );
    assert_eq!(
        map(LiveEvent::Error {
            error: LiveError::RateLimited,
            fatal: false
        }),
        vec![Outbound::Json(ServerFrame::Error {
            code: "rate_limited".into(),
            message: LiveError::RateLimited.to_string(),
            fatal: false
        })]
    );
    assert_eq!(
        map(LiveEvent::Closed(CloseReason::Client)),
        vec![Outbound::Json(ServerFrame::Closed {
            reason: "client".into()
        })]
    );
    assert_eq!(
        map(LiveEvent::Closed(CloseReason::Remote {
            code: Some(1000),
            reason: "bye".into()
        })),
        vec![Outbound::Json(ServerFrame::Closed {
            reason: "remote (1000) bye".into()
        })]
    );
    assert_eq!(
        map(LiveEvent::Closed(CloseReason::Remote {
            code: None,
            reason: String::new()
        })),
        vec![Outbound::Json(ServerFrame::Closed {
            reason: "remote".into()
        })]
    );
    let out = map(LiveEvent::Closed(CloseReason::Error(
        LiveError::Unauthorized,
    )));
    assert_eq!(out.len(), 2);
    assert!(
        matches!(&out[0], Outbound::Json(ServerFrame::Error { code, fatal: true, .. }) if code == "unauthorized")
    );
    // Tool calls surface through ToolStarted/ToolFinished instead.
    assert!(map(LiveEvent::ToolCall(ToolCall {
        call_id: "c".into(),
        name: "n".into(),
        args: serde_json::json!({})
    }))
    .is_empty());
}

#[test]
fn maps_tool_progress() {
    assert_eq!(
        map_event(
            LiveAgentEvent::ToolStarted {
                call_id: "c".into(),
                name: "n".into()
            },
            "s",
            "p",
            "t"
        ),
        vec![Outbound::Json(ServerFrame::ToolStarted {
            call_id: "c".into(),
            name: "n".into()
        })]
    );
    assert_eq!(
        map_event(
            LiveAgentEvent::ToolFinished {
                call_id: "c".into(),
                name: "n".into(),
                is_error: true,
                cancelled: false,
                duration: Duration::from_millis(5)
            },
            "s",
            "p",
            "t"
        ),
        vec![Outbound::Json(ServerFrame::ToolFinished {
            call_id: "c".into(),
            name: "n".into(),
            ok: false,
            cancelled: false
        })]
    );
}

#[test]
fn parses_only_a_start_frame_first() {
    let start = parse_start(&Message::Text(
        r#"{"type":"start","provider":"sarvam","thread_id":"t","client_id":"c"}"#.into(),
    ))
    .unwrap();
    assert_eq!(start.provider.as_deref(), Some("sarvam"));
    assert_eq!(start.client_id.as_deref(), Some("c"));
    assert_eq!(start.input_sample_rate, 16_000);
    assert!(parse_start(&Message::Text(r#"{"type":"stop"}"#.into())).is_err());
    assert!(parse_start(&Message::Text("nope".into())).is_err());
    assert!(parse_start(&Message::Binary(Bytes::new())).is_err());
}
