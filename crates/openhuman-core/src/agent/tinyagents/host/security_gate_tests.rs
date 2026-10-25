use super::*;
use crate::security::policy::AutonomyLevel;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Mutex;
use tinyagents_harness::host::security_gate::{ContentOrigin, ScreenOutcome, SecurityGate};
use tinytools::ToolResult;

/// Minimal registered tool. `execute` is unreachable: this adapter answers
/// questions about tools, it never runs them.
struct FakeTool {
    name: &'static str,
    permission: PermissionLevel,
    external: bool,
}

#[async_trait]
impl Tool for FakeTool {
    fn name(&self) -> &str {
        self.name
    }
    fn description(&self) -> &str {
        "test tool"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({ "type": "object" })
    }
    fn permission_level(&self) -> PermissionLevel {
        self.permission
    }
    fn external_effect(&self) -> bool {
        self.external
    }
    async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<ToolResult> {
        unreachable!("the security gate never executes a tool")
    }
}

fn tool(name: &'static str, permission: PermissionLevel, external: bool) -> Box<dyn Tool> {
    Box::new(FakeTool {
        name,
        permission,
        external,
    })
}

fn registry() -> Vec<Arc<Vec<Box<dyn Tool>>>> {
    vec![Arc::new(vec![
        tool("read_file", PermissionLevel::ReadOnly, false),
        tool("write_file", PermissionLevel::Write, false),
        tool("spawn_subagent", PermissionLevel::Execute, false),
        tool(SHELL_TOOL, PermissionLevel::Execute, false),
    ])]
}

fn policy(autonomy: AutonomyLevel) -> Arc<SecurityPolicy> {
    Arc::new(SecurityPolicy {
        autonomy,
        ..SecurityPolicy::default()
    })
}

fn gate(autonomy: AutonomyLevel) -> OpenHumanSecurityGate {
    OpenHumanSecurityGate::new(policy(autonomy), registry())
}

#[cfg(feature = "modules")]
#[derive(Debug, Default)]
struct ScriptedCommandGateway(Mutex<Vec<(String, crate::security::policy::CommandStage)>>);

#[cfg(feature = "modules")]
impl crate::security::policy::Sealed for ScriptedCommandGateway {}

#[cfg(feature = "modules")]
#[async_trait]
impl crate::security::policy::CommandGateway for ScriptedCommandGateway {
    async fn decide(
        &self,
        _policy: &SecurityPolicy,
        query: &crate::security::policy::CommandQuery<'_>,
    ) -> Result<crate::security::policy::CommandVerdict, crate::security::policy::Refusal> {
        self.0
            .lock()
            .unwrap()
            .push((query.command.to_owned(), query.stage));
        Ok(crate::security::policy::CommandVerdict {
            class: crate::security::policy::CommandClass::Read,
            gate: PolicyGateDecision::Allow,
        })
    }
}

#[cfg(feature = "modules")]
fn gate_with_command_gateway(
    autonomy: AutonomyLevel,
    gateway: Arc<ScriptedCommandGateway>,
) -> OpenHumanSecurityGate {
    let mut policy = SecurityPolicy {
        autonomy,
        ..SecurityPolicy::default()
    };
    policy.command_gateway = Some(gateway);
    OpenHumanSecurityGate::new(Arc::new(policy), registry())
}

fn req(tool_name: &str, args: serde_json::Value) -> ToolCallRequest {
    ToolCallRequest::new(tool_name, args, "lead")
}

#[cfg(feature = "modules")]
fn shell_context(
    args: &serde_json::Value,
    class: crate::security::policy::CommandClass,
    hidden_execution: bool,
    literal_words: &[&str],
) -> serde_json::Value {
    shell_context_for_args(args, args, class, hidden_execution, literal_words)
}

#[cfg(feature = "modules")]
fn shell_context_for_args(
    request_arguments: &serde_json::Value,
    effective_arguments: &serde_json::Value,
    class: crate::security::policy::CommandClass,
    hidden_execution: bool,
    literal_words: &[&str],
) -> serde_json::Value {
    use crate::security::policy::{CommandAnalysis, CommandSegment};

    let analysis = CommandAnalysis {
        segments: vec![CommandSegment {
            source: "command".into(),
            command: "command".into(),
            basename: "command".into(),
            normalized_name: "command".into(),
            arguments: Vec::new(),
            class,
            leading_env_assignment: false,
            dangerous_env_prefix: false,
            executor: false,
        }],
        hidden_execution,
        redirection: false,
        expansion: false,
        tee: false,
        background: false,
        literal_words: literal_words
            .iter()
            .map(|word| (*word).to_owned())
            .collect(),
    };
    json!({
        "tool_name": SHELL_TOOL,
        "request_arguments": request_arguments,
        "effective_arguments": effective_arguments,
        "tool_facts": {"analysis": analysis},
    })
}

#[cfg(feature = "modules")]
fn shell_analysis(
    class: crate::security::policy::CommandClass,
    hidden_execution: bool,
    literal_words: &[&str],
) -> serde_json::Value {
    shell_context(
        &json!({"command": "fixture"}),
        class,
        hidden_execution,
        literal_words,
    )["tool_facts"]["analysis"]
        .clone()
}

#[tokio::test]
async fn there_is_no_audit_id_to_drain_when_nothing_was_prompted() {
    // No approval gate is installed in tests, so nothing parks and nothing
    // is stashed. The point of the assertion is that `take_audit_request_id`
    // is a drain, not a source: a caller can never mistake "not prompted"
    // for "there is a row to close", which is what would push it into
    // issuing a second `intercept_audited` and raising a duplicate card.
    let gate = gate(AutonomyLevel::Full);
    let _ = gate
        .authorize_tool(&req("read_file", json!({ "path": "notes.md" })))
        .await
        .expect("authorize");

    assert_eq!(gate.take_audit_request_id("any-call-id"), None);
}

#[test]
fn draining_an_audit_id_yields_it_exactly_once() {
    // An id is valid for exactly one `record_execution`; handing the same
    // one out twice would write two terminal rows for a single approval.
    let gate = gate(AutonomyLevel::Full);
    gate.pending_audit
        .lock()
        .expect("uncontended")
        .insert("call-1".to_string(), "request-9".to_string());

    assert_eq!(
        gate.take_audit_request_id("call-1"),
        Some("request-9".to_string())
    );
    assert_eq!(gate.take_audit_request_id("call-1"), None);
}

#[tokio::test]
async fn a_read_only_tool_is_allowed_in_every_tier() {
    for tier in [
        AutonomyLevel::ReadOnly,
        AutonomyLevel::Supervised,
        AutonomyLevel::Full,
    ] {
        let decision = gate(tier)
            .authorize_tool(&req("read_file", json!({ "path": "notes.md" })))
            .await
            .unwrap();
        assert_eq!(decision, GateDecision::Allow, "tier {tier:?}");
    }
}

async fn cron_decision(gate: OpenHumanSecurityGate, tool_name: &str) -> GateDecision {
    let core =
        crate::core::runtime::CoreContext::for_test(crate::core::runtime::DomainSet::full(), None);
    let origin = crate::core::runtime::turn_origin::AgentTurnOrigin::TrustedAutomation {
        job_id: "cron-security-test".into(),
        source: crate::core::runtime::turn_origin::TrustedAutomationSource::Cron,
    };
    crate::core::runtime::CoreContext::scope_with_turn_origin(
        core,
        Some(origin),
        gate.authorize_tool(&req(tool_name, json!({}))),
    )
    .await
    .expect("authorization")
}

#[tokio::test]
async fn cron_headless_settlement_allows_only_spawn_control() {
    let ordinary = cron_decision(gate(AutonomyLevel::Full), "spawn_subagent").await;
    assert!(!ordinary.is_allowed(), "ordinary Cron remains read-only");

    let opted_in = cron_decision(
        gate(AutonomyLevel::Full).with_headless_descendant_settlement(true),
        "spawn_subagent",
    )
    .await;
    assert_eq!(opted_in, GateDecision::Allow);

    let read_only = cron_decision(
        OpenHumanSecurityGate::new(policy(AutonomyLevel::ReadOnly), registry())
            .with_headless_descendant_settlement(true),
        "spawn_subagent",
    )
    .await;
    assert!(
        !read_only.is_allowed(),
        "headless settlement must not override the agent's autonomy tier"
    );

    let write = cron_decision(
        gate(AutonomyLevel::Full).with_headless_descendant_settlement(true),
        "write_file",
    )
    .await;
    assert!(!write.is_allowed(), "child action tools remain read-only");

    let shell = cron_decision(
        gate(AutonomyLevel::Full).with_headless_descendant_settlement(true),
        SHELL_TOOL,
    )
    .await;
    assert!(!shell.is_allowed(), "shell stays denied for opted-in Cron");
}

#[tokio::test]
async fn cron_headless_settlement_does_not_allow_effectful_spawn_aliases() {
    let effectful = Arc::new(vec![tool("spawn_subagent", PermissionLevel::Execute, true)]);
    let decision = cron_decision(
        OpenHumanSecurityGate::new(policy(AutonomyLevel::Full), vec![effectful])
            .with_headless_descendant_settlement(true),
        "spawn_subagent",
    )
    .await;
    assert!(
        !decision.is_allowed(),
        "external-effect classification wins"
    );
}

#[tokio::test]
async fn read_only_autonomy_denies_an_acting_tool() {
    let decision = gate(AutonomyLevel::ReadOnly)
        .authorize_tool(&req("write_file", json!({ "path": "a.txt" })))
        .await
        .unwrap();
    assert!(!decision.is_allowed());
    let reason = decision.denial_reason().expect("a Deny carries a reason");
    assert!(
        reason.contains(tinytools::status::POLICY_BLOCKED_MARKER),
        "the machine-recognisable marker must survive rendering: {reason}"
    );
}

#[tokio::test]
async fn an_unregistered_tool_is_denied_not_allowed() {
    // Fail-closed: without the registered Tool there is no permission level
    // to reason about, so "unknown" must never mean "fine".
    let decision = gate(AutonomyLevel::Full)
        .authorize_tool(&req("definitely_not_a_tool", json!({})))
        .await
        .unwrap();
    assert!(!decision.is_allowed());
    assert!(decision
        .denial_reason()
        .expect("denial reason")
        .contains("definitely_not_a_tool"));
}

#[tokio::test]
async fn an_empty_registry_denies_everything() {
    let gate = OpenHumanSecurityGate::new(policy(AutonomyLevel::Full), Vec::new());
    assert!(!gate
        .authorize_tool(&req("read_file", json!({})))
        .await
        .unwrap()
        .is_allowed());
}

#[tokio::test]
#[cfg(feature = "modules")]
async fn a_read_class_shell_command_runs_without_a_prompt() {
    let args = json!({"command": "ls -la"});
    let decision = gate(AutonomyLevel::Supervised)
        .authorize_tool_with_context(
            &req(SHELL_TOOL, args.clone()),
            Some(&shell_context(
                &args,
                crate::security::policy::CommandClass::Read,
                false,
                &["ls", "-la"],
            )),
        )
        .await
        .unwrap();
    assert_eq!(decision, GateDecision::Allow);
}

#[tokio::test]
#[cfg(feature = "modules")]
async fn shell_authorization_uses_command_without_tinybox_facts() {
    let gateway = Arc::new(ScriptedCommandGateway::default());
    let gate = gate_with_command_gateway(AutonomyLevel::Full, gateway.clone());
    let decision = gate
        .authorize_tool(&req(SHELL_TOOL, json!({"command": "ls -la"})))
        .await
        .expect("authorization result");
    assert!(decision.is_allowed());
    assert_eq!(
        gateway.0.lock().unwrap().as_slice(),
        &[(
            "ls -la".into(),
            crate::security::policy::CommandStage::Authorize
        )]
    );
}

#[tokio::test]
#[cfg(feature = "modules")]
async fn shell_authorization_ignores_stale_facts_and_uses_request_command() {
    let gateway = Arc::new(ScriptedCommandGateway::default());
    let gate = gate_with_command_gateway(AutonomyLevel::Full, gateway.clone());
    let args = json!({"command": "ls -la"});
    let analysis = crate::security::policy::CommandAnalysis {
        segments: vec![],
        hidden_execution: false,
        redirection: false,
        expansion: false,
        tee: false,
        background: false,
        literal_words: vec!["ls".to_owned()],
    };
    let context = json!({
        "tool_name": "shell",
        "request_arguments": {"command": "other command"},
        "effective_arguments": {"command": "cat ~/.ssh/id_rsa"},
        "tool_facts": {"analysis": analysis},
    });
    let decision = gate
        .authorize_tool_with_context(&req(SHELL_TOOL, args), Some(&context))
        .await
        .expect("authorization result");
    assert!(decision.is_allowed());
    assert_eq!(gateway.0.lock().unwrap()[0].0, "ls -la");
}

#[test]
#[cfg(feature = "modules")]
fn shell_policy_uses_the_effective_arguments_bound_by_the_harness() {
    let request_arguments = json!({"command": "touch output"});
    let effective_arguments = json!({
        "command": "touch output",
        "category": "destructive"
    });
    let context = shell_context_for_args(
        &request_arguments,
        &effective_arguments,
        crate::security::policy::CommandClass::Write,
        false,
        &["touch", "output"],
    );
    let call = req(SHELL_TOOL, request_arguments);
    let (effective, analysis) =
        shell_analysis_for_call(&call, Some(&context)).expect("call-bound facts");

    assert_eq!(effective, effective_arguments);
    assert_eq!(
        OpenHumanSecurityGate::shell_decision(&SecurityPolicy::default(), &effective, &analysis),
        Ok(PolicyGateDecision::Prompt)
    );
}

#[tokio::test]
#[cfg(feature = "modules")]
async fn a_read_class_shell_command_still_runs_in_read_only_mode() {
    // The regression the stage ordering exists for: `shell` declares the
    // coarse `Execute` permission level, so a naive tier check would refuse
    // `ls` in read-only mode — which OpenHuman's own ShellTool permits,
    // because `classify_command` says the command itself is a Read.
    let args = json!({"command": "ls -la"});
    let decision = gate(AutonomyLevel::ReadOnly)
        .authorize_tool_with_context(
            &req(SHELL_TOOL, args.clone()),
            Some(&shell_context(
                &args,
                crate::security::policy::CommandClass::Read,
                false,
                &["ls", "-la"],
            )),
        )
        .await
        .unwrap();
    assert_eq!(decision, GateDecision::Allow);
}

#[tokio::test]
#[cfg(feature = "modules")]
async fn a_write_shell_command_is_blocked_outright_in_read_only() {
    let args = json!({"command": "rm -rf /tmp/x"});
    let decision = gate(AutonomyLevel::ReadOnly)
        .authorize_tool_with_context(
            &req(SHELL_TOOL, args.clone()),
            Some(&shell_context(
                &args,
                crate::security::policy::CommandClass::Write,
                false,
                &["rm", "-rf", "/tmp/x"],
            )),
        )
        .await
        .unwrap();
    assert!(!decision.is_allowed());
    assert!(decision
        .denial_reason()
        .expect("denial reason")
        .contains(tinytools::status::POLICY_BLOCKED_MARKER));
}

#[tokio::test]
#[cfg(feature = "modules")]
async fn hidden_execution_is_blocked_below_the_full_tier() {
    // `check_gated_command`'s structural guard: a substitution could smuggle
    // an unseen command past the approval the human actually read.
    let args = json!({"command": "echo $(whoami)"});
    let decision = gate(AutonomyLevel::Supervised)
        .authorize_tool_with_context(
            &req(SHELL_TOOL, args.clone()),
            Some(&shell_context(
                &args,
                crate::security::policy::CommandClass::Write,
                true,
                &["echo"],
            )),
        )
        .await
        .unwrap();
    assert!(!decision.is_allowed());
}

#[test]
#[cfg(feature = "modules")]
fn the_declared_category_can_only_raise_the_command_class() {
    let full = SecurityPolicy {
        autonomy: AutonomyLevel::Full,
        ..SecurityPolicy::default()
    };
    // `touch` is Write, which Full runs silently…
    assert_eq!(
        OpenHumanSecurityGate::shell_decision(
            &full,
            &json!({ "command": "touch f" }),
            &shell_analysis(
                crate::security::policy::CommandClass::Write,
                false,
                &["touch", "f"],
            ),
        ),
        Ok(PolicyGateDecision::Allow)
    );
    // …until the model itself declares it destructive, which must prompt.
    assert_eq!(
        OpenHumanSecurityGate::shell_decision(
            &full,
            &json!({ "command": "touch f", "category": "destructive" }),
            &shell_analysis(
                crate::security::policy::CommandClass::Write,
                false,
                &["touch", "f"],
            )
        ),
        Ok(PolicyGateDecision::Prompt)
    );
    // The hint can never lower the deterministic floor: a network command
    // declared "read" still prompts.
    assert_eq!(
        OpenHumanSecurityGate::shell_decision(
            &full,
            &json!({ "command": "curl https://example.invalid", "category": "read" }),
            &shell_analysis(
                crate::security::policy::CommandClass::Network,
                false,
                &["curl", "https://example.invalid"],
            )
        ),
        Ok(PolicyGateDecision::Prompt)
    );
}

#[test]
#[cfg(feature = "modules")]
fn a_missing_command_argument_classifies_as_read_rather_than_panicking() {
    let supervised = SecurityPolicy::default();
    assert_eq!(
        OpenHumanSecurityGate::shell_decision(
            &supervised,
            &json!({}),
            &shell_analysis(crate::security::policy::CommandClass::Read, false, &[],),
        ),
        Ok(PolicyGateDecision::Allow)
    );
}

#[path = "security_gate_screen_and_policy_tests.rs"]
mod screen_and_policy;
