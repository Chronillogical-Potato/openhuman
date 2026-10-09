use super::*;

#[test]
fn each_origin_maps_to_its_thread_and_label() {
    let web = AgentTurnOrigin::WebChat {
        thread_id: "t1".into(),
        client_id: "c".into(),
        request_id: None,
    };
    assert_eq!(
        describe_origin(Some(&web)),
        (Some("t1".into()), Some("web_chat".into()))
    );
    let channel = AgentTurnOrigin::ExternalChannel {
        channel: "telegram".into(),
        sender: None,
        sender_name: None,
        history_key: None,
        reply_target: "r".into(),
        message_id: "m".into(),
    };
    assert_eq!(
        describe_origin(Some(&channel)).1.as_deref(),
        Some("channel:telegram")
    );
    let cron = AgentTurnOrigin::TrustedAutomation {
        job_id: "j".into(),
        source: TrustedAutomationSource::Cron,
    };
    assert_eq!(describe_origin(Some(&cron)).1.as_deref(), Some("cron"));
    assert_eq!(
        describe_origin(Some(&AgentTurnOrigin::Cli)).1.as_deref(),
        Some("cli")
    );
    assert_eq!(describe_origin(None), (None, None));
    assert_eq!(
        describe_origin(Some(&AgentTurnOrigin::Unknown)),
        (None, None)
    );
}

#[tokio::test]
async fn ambient_scope_reads_the_turn_and_prefers_the_subagent() {
    use crate::memory::scope::{within, MemoryIdentity};
    let origin = AgentTurnOrigin::WebChat {
        thread_id: "thread-9".into(),
        client_id: "c".into(),
        request_id: None,
    };
    let (parent, child) = crate::agent::turn_origin::with_origin(origin, async {
        within(MemoryIdentity::agent("orchestrator"), async {
            (
                UsageScope::ambient(Some("openhuman"), None),
                UsageScope::ambient(Some("openhuman"), Some(("researcher", "task-1"))),
            )
        })
        .await
    })
    .await;
    assert_eq!(parent.thread_id.as_deref(), Some("thread-9"));
    assert_eq!(parent.origin.as_deref(), Some("web_chat"));
    assert_eq!(parent.agent_id.as_deref(), Some("orchestrator"));
    assert_eq!(parent.provider.as_deref(), Some("openhuman"));
    assert_eq!(parent.subagent_task_id, None);
    assert_eq!(child.agent_id.as_deref(), Some("researcher"));
    assert_eq!(child.subagent_task_id.as_deref(), Some("task-1"));
}

#[test]
fn outside_any_turn_and_without_a_provider_the_scope_is_empty() {
    let scope = UsageScope::ambient(Some(""), None);
    assert!(scope.is_empty(), "{scope:?}");
}
