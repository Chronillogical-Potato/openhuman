use super::*;
use crate::agent::turn_origin::with_origin;
use crate::security::approval::{ApprovalChatContext, APPROVAL_CHAT_CONTEXT};

#[tokio::test]
async fn non_interactive_origin_auto_approves() {
    let tool = RequestPlanReviewTool::new();
    let out = with_origin(
        AgentTurnOrigin::Cli,
        tool.execute(json!({ "summary": "do x", "steps": ["a", "b"] })),
    )
    .await
    .unwrap();
    assert!(!out.is_error);
    assert!(out.output().starts_with("approved"));
}

#[tokio::test]
async fn interactive_turn_parks_until_resolved() {
    // The tool is inert (no-ops rather than parks) outside Plan mode — see
    // `plan_mode_is_inert_outside_plan_mode` below — so this thread must be
    // in Plan mode for the park to actually happen.
    crate::agent::tinyagents::run_mode::set_mode(
        "t-int",
        tinyagents_harness::middleware::RunMode::Plan,
    );
    let tool = RequestPlanReviewTool::new();
    let fut = with_origin(
        AgentTurnOrigin::WebChat {
            thread_id: "t-int".into(),
            client_id: "c-int".into(),
            request_id: Some("req-int".into()),
        },
        APPROVAL_CHAT_CONTEXT.scope(
            ApprovalChatContext {
                thread_id: "t-int".into(),
                client_id: "c-int".into(),
                request_id: Some("req-int".into()),
            },
            tool.execute(json!({ "summary": "plan", "steps": ["one"] })),
        ),
    );
    // The web channel supplies both task locals. One poll reaches the gate
    // and must remain pending; no wall-clock timeout is needed to prove park.
    tokio::pin!(fut);
    assert!(
        matches!(futures::poll!(fut.as_mut()), std::task::Poll::Pending),
        "interactive turn should park, not resolve immediately"
    );
}

#[tokio::test]
async fn plan_mode_is_inert_outside_plan_mode() {
    // A thread that has never toggled plan mode defaults to Build — the
    // tool must no-op (return immediately) rather than park.
    let tool = RequestPlanReviewTool::new();
    let out = with_origin(
        AgentTurnOrigin::WebChat {
            thread_id: "t-build-inert".into(),
            client_id: "c-build-inert".into(),
            request_id: Some("req-build-inert".into()),
        },
        tool.execute(json!({ "summary": "plan", "steps": ["one"] })),
    )
    .await
    .unwrap();
    assert!(!out.is_error);
    assert!(out.output().starts_with("not applicable"));
}
