use super::*;
use crate::core::runtime::{ContextOverlay, CoreContext, DomainSet};
use crate::security::SecurityPolicy;

fn agent_ctx(
    id: &str,
    policy: Option<SecurityPolicy>,
    approvals_disabled: bool,
) -> Arc<CoreContext> {
    let parent = CoreContext::for_test(DomainSet::full(), Some(std::env::temp_dir()));
    let mut overlay = ContextOverlay::new(
        Config::default(),
        DomainSet::kernel(),
        crate::tools::toolpacks::ToolGroups::none(),
    )
    .session_agent(id);
    overlay.agent_policy = policy.map(Arc::new);
    overlay.approvals_disabled = approvals_disabled;
    parent.derive_with(overlay)
}

/// Runs one gated call under `ctx` and `origin`; `None` means it parked.
async fn gated_call(
    gate: &ApprovalGate,
    ctx: Arc<CoreContext>,
    origin: AgentTurnOrigin,
    tool: &str,
) -> Option<GateOutcome> {
    CoreContext::scope(
        ctx,
        turn_origin::with_origin(
            origin,
            APPROVAL_CHAT_CONTEXT.scope(
                chat_ctx(),
                gate.intercept_audited_bounded(
                    tool,
                    "run it",
                    serde_json::json!({}),
                    Some(Duration::from_millis(100)),
                ),
            ),
        ),
    )
    .await
    .map(|(outcome, _)| outcome)
}

#[tokio::test]
async fn an_agent_with_the_gate_off_runs_unparked_while_a_sibling_parks() {
    let (gate, _dir) = test_gate();

    let open = gated_call(
        &gate,
        agent_ctx("gate-off", None, true),
        web_origin(),
        "shell",
    )
    .await;
    assert!(matches!(open, Some(GateOutcome::Allow)), "got {open:?}");

    let parked = gated_call(
        &gate,
        agent_ctx("gate-on", None, false),
        web_origin(),
        "shell",
    )
    .await;
    assert!(parked.is_none(), "the sibling must park, got {parked:?}");
}

#[tokio::test]
async fn the_gate_off_switch_still_refuses_an_unlabelled_origin() {
    let (gate, _dir) = test_gate();
    let outcome = gated_call(
        &gate,
        agent_ctx("gate-off-unknown", None, true),
        AgentTurnOrigin::Unknown,
        "shell",
    )
    .await;
    assert!(
        matches!(outcome, Some(GateOutcome::Deny { .. })),
        "got {outcome:?}"
    );
}

#[tokio::test]
async fn each_agent_answers_from_its_own_auto_approve_list() {
    let (gate, _dir) = test_gate();
    let listed = SecurityPolicy {
        auto_approve: vec!["shell".into()],
        auto_approve_all: false,
        ..SecurityPolicy::default()
    };
    let unlisted = SecurityPolicy {
        auto_approve: Vec::new(),
        auto_approve_all: false,
        ..SecurityPolicy::default()
    };

    let allowed = gated_call(
        &gate,
        agent_ctx("listed", Some(listed), false),
        web_origin(),
        "shell",
    )
    .await;
    assert!(
        matches!(allowed, Some(GateOutcome::Allow)),
        "got {allowed:?}"
    );

    let parked = gated_call(
        &gate,
        agent_ctx("unlisted", Some(unlisted), false),
        web_origin(),
        "shell",
    )
    .await;
    assert!(parked.is_none(), "an agent without the grant must park");
}

#[tokio::test]
async fn an_agent_without_auto_approve_all_parks_under_a_process_policy_that_has_it() {
    let (gate, dir) = test_gate();
    let process = Arc::new(SecurityPolicy {
        auto_approve_all: true,
        ..SecurityPolicy::default()
    });
    let _guard = crate::security::live_policy::install_scoped(
        process,
        dir.path().to_path_buf(),
        dir.path().to_path_buf(),
    );

    let booted = gated_call(
        &gate,
        CoreContext::for_test(DomainSet::full(), Some(dir.path().to_path_buf())),
        web_origin(),
        "shell",
    )
    .await;
    assert!(matches!(booted, Some(GateOutcome::Allow)), "got {booted:?}");

    let strict = SecurityPolicy {
        auto_approve: Vec::new(),
        auto_approve_all: false,
        ..SecurityPolicy::default()
    };
    let parked = gated_call(
        &gate,
        agent_ctx("strict", Some(strict), false),
        web_origin(),
        "shell",
    )
    .await;
    assert!(
        parked.is_none(),
        "the agent's own policy must win over the process blanket approval"
    );
}
