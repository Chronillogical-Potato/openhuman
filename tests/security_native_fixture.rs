use axum::{body::{to_bytes, Body}, http::{Request, StatusCode}, Router};
use openhuman_core::{
    config::Config,
    core::runtime::{turn_origin::{self, AgentTurnOrigin}, DispatchCtx},
    modules::security::{client, reinitialize},
    sandbox::types::{SandboxBackendKind, SandboxPolicy},
    security::approval::{gate::ApprovalGate, ApprovalChatContext, ApprovalTurnContext, GateOutcome},
};
use std::path::PathBuf;
use tinysecurity_bus::{
    CommandAutonomy, CommandClass, CommandExecutionMode, CommandPolicy, CommandRequest,
    ProtectedPathRequest2, RedactMode,
};
use serde_json::{json, Value};
use tower::ServiceExt;

const TOKEN: &str = "security-native-fixture-token";

async fn rpc(router: &Router, method: &str, params: Value) -> Value {
    let body = json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string();
    let response = router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/rpc")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {TOKEN}"))
            .body(Body::from(body))
            .expect("valid request"),
    ).await.expect("router response");
    assert_eq!(response.status(), StatusCode::OK, "{method}");
    serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.expect("response body"))
        .expect("JSON-RPC response")
}

fn rpc_result(response: &Value) -> &Value {
    assert!(response.get("error").is_none(), "{response}");
    let result = &response["result"];
    if result.get("logs").is_some() { &result["result"] } else { result }
}

/// Loads the published TinySecurity archive through the compiled registry pin.
/// This stays as a separate integration target so its focused native check
/// does not compile unrelated core unit-test modules.
#[tokio::test]
#[ignore = "downloads and loads the pinned native TinySecurity release"]
async fn native_fixture_exercises_the_pinned_release_contracts() {
    let scratch = tempfile::tempdir().expect("scratch workspace");
    std::env::set_var("OPENHUMAN_WORKSPACE", scratch.path());
    std::env::set_var("OPENHUMAN_KEYRING_BACKEND", "file");
    let mut config = Config::default();
    config.modules.enabled = true;
    config.workspace_dir = scratch.path().to_path_buf();
    config.config_path = scratch.path().join("config.toml");
    config.action_dir = scratch.path().join("action");
    config.action_dir_override = Some(config.action_dir.clone());
    config.save().await.expect("save fixture config");
    openhuman_core::security::secret_storage::init_workspace(scratch.path());

    let client = client(&config)
        .await
        .expect("pinned TinySecurity module should load on supported native runners");
    let info = client
        .policy_info()
        .await
        .expect("the loaded module should answer its typed policy-info contract");

    assert_eq!(info.contract_version, (1, 33));

    reinitialize(&config)
        .await
        .expect("the ready module should accept the same trusted host on reinitialization");
    let reinitialized_info = client
        .policy_info()
        .await
        .expect("the reinitialized module should continue answering policy info");
    assert_eq!(reinitialized_info.contract_version, (1, 33));

    let redacted = client
        .redact(RedactMode::Text, "token=sk-ant-example-secret-value")
        .await
        .expect("redaction should be available from the released module");
    assert!(redacted.changed, "credential text must be redacted");
    assert!(!redacted.text.contains("sk-ant-example-secret-value"));

    let floor = client
        .protected_path2(ProtectedPathRequest2 {
            paths: vec![PathBuf::from("../outside"), PathBuf::from("ordinary.txt")],
        })
        .await
        .expect("the unconditional path floor should answer");
    assert_eq!(floor.protected, vec![true, false]);

    let command_scope = client
        .register_command_policy(CommandPolicy {
            enabled: true,
            execution_mode: CommandExecutionMode::HarnessGated,
            shell_dialect: tinysecurity_bus::ShellDialect::default(),
            autonomy: CommandAutonomy::Full,
            allowed_commands: vec![],
            max_actions_per_hour: 100,
            require_approval_for_medium_risk: true,
            block_high_risk_commands: false,
            action_dir: std::env::current_dir().expect("fixture action dir"),
            home_dir: None,
        })
        .await
        .expect("host should register command policy");
    let class = client
        .classify_command(CommandRequest {
            policy_id: command_scope,
            command: "curl https://example.invalid".into(),
            declared_class: None,
        })
        .await
        .expect("shell command classification should answer");
    assert_eq!(class.class, CommandClass::Network);

    let keyring = client
        .keyring_access2(tinysecurity_bus::KeyringRequest2 {
            caller: tinysecurity_bus::CallerContext {
                agent: "fixture-agent".to_owned(),
                ..Default::default()
            },
            purpose: "native fixture denial check".to_owned(),
            operation: tinysecurity_bus::KeyringOperation2::Get {
                name: "fixture-secret".to_owned(),
            },
        })
        .await;
    assert!(keyring.is_err(), "keyring access without host consent must fail closed");
}

/// A corrupted library presented through the explicit test override must never
/// produce a usable client. This second process is invoked by the fixture script.
#[tokio::test]
#[ignore = "loads a deliberately corrupted copy of the pinned native module"]
async fn native_fixture_rejects_a_tampered_module() {
    let mut config = Config::default();
    config.modules.enabled = true;
    assert!(
        client(&config).await.is_err(),
        "tampered TinySecurity must fail closed during module admission"
    );
}

/// Denying the host consent event must stop a real module keyring call before
/// any platform keyring backend is touched.
#[tokio::test]
#[ignore = "loads the pinned module and round trips the host keyring consent event"]
async fn native_fixture_denies_keyring_before_backend_access() {
    let scratch = tempfile::tempdir().expect("scratch workspace");
    std::env::set_var("OPENHUMAN_WORKSPACE", scratch.path());
    std::env::set_var("OPENHUMAN_KEYRING_BACKEND", "file");
    openhuman_core::core::auth::init_rpc_token_with_value(TOKEN).expect("install RPC token");
    let router = openhuman_rpc::server::build_core_http_router(false);
    let (tx, rx) = tokio::sync::oneshot::channel();
    let tx = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));
    let _subscription = openhuman_core::core::bus::BUS.get()
        .expect("RPC router initializes the event bus")
        .on("security-native-keyring", move |event| {
        let tx = std::sync::Arc::clone(&tx);
        async move {
            if let openhuman_core::core::events::DomainEvent::KeyringAccessApprovalRequested(request) = event {
                if let Some(tx) = tx.lock().expect("event sender mutex").take() {
                    let _ = tx.send(request.request_id);
                }
            }
        }
        });

    let mut config = Config::default();
    config.modules.enabled = true;
    let client = client(&config).await.expect("released module loads");
    let pending = tokio::spawn(async move {
        client.keyring_access2(tinysecurity_bus::KeyringRequest2 {
            caller: tinysecurity_bus::CallerContext {
                agent: "native-fixture".into(),
                call_id: "native-fixture-keyring-call".into(),
                ..Default::default()
            },
            purpose: "verify denial before OS keyring access".into(),
            operation: tinysecurity_bus::KeyringOperation2::Get { name: "fixture-secret".into() },
        }).await
    });
    let request_id = tokio::time::timeout(std::time::Duration::from_secs(10), rx)
        .await.expect("module should request consent").expect("consent event");
    let response = rpc(&router, "openhuman.keyring_consent_respond_operation", json!({
        "request_id": request_id,
        "approved": false,
    })).await;
    assert!(response.get("error").is_none(), "host should accept the denial: {response}");
    assert!(pending.await.expect("keyring request task").is_err(), "denial must fail closed");
}

/// Runs a harmless shell command through the public host adapter so the
/// released module plans and consumes a sandbox operation on every CI OS.
#[tokio::test]
#[ignore = "loads the pinned module and exercises the sandbox plan/consume path"]
async fn native_fixture_executes_through_the_module_sandbox_plan() {
    let scratch = tempfile::tempdir().expect("scratch workspace");
    std::env::set_var("OPENHUMAN_WORKSPACE", scratch.path());
    std::env::set_var("OPENHUMAN_ACTION_DIR", scratch.path());
    let mut config = Config::default();
    config.modules.enabled = true;
    config.workspace_dir = scratch.path().to_path_buf();
    config.config_path = scratch.path().join("config.toml");
    config.action_dir = scratch.path().to_path_buf();
    config.action_dir_override = Some(config.action_dir.clone());
    config.save().await.expect("save sandbox fixture config");
    client(&config).await.expect("released module loads");
    let policy = SandboxPolicy {
        backend: SandboxBackendKind::None,
        workspace_root: scratch.path().to_path_buf(),
        state_dir: scratch.path().join("state"),
        read_only_mounts: vec![],
        read_write_mounts: vec![],
        allow_network: false,
        mount_workspace: true,
        env_passthrough: vec![],
        docker_overrides: None,
    };
    let command = if cfg!(windows) { "echo sandbox-fixture" } else { "echo sandbox-fixture" };
    let result = turn_origin::with_origin(
        AgentTurnOrigin::Cli,
        openhuman_core::sandbox::ops::execute_in_sandbox(
            &DispatchCtx::process_default(),
            &policy,
            command,
            scratch.path(),
            std::collections::HashMap::new(),
            std::time::Duration::from_secs(10),
        ),
    )
    .await
    .expect("TinySecurity should plan and consume the safe sandbox command");
    assert!(result.success(), "sandbox command failed: {}", result.stderr);
    assert!(result.stdout.contains("sandbox-fixture"));
    let outside = tempfile::tempdir().expect("outside directory");
    let denied = turn_origin::with_origin(
        AgentTurnOrigin::Cli,
        openhuman_core::sandbox::ops::execute_in_sandbox(
            &DispatchCtx::process_default(),
            &policy,
            command,
            outside.path(),
            std::collections::HashMap::new(),
            std::time::Duration::from_secs(10),
        ),
    )
    .await;
    assert!(denied.is_err(), "sandbox path policy must refuse a working directory outside its workspace");
}

/// Parks a real approval request, approves it through the host RPC controller,
/// and checks that the caller resumes with an allow outcome.
#[tokio::test]
#[ignore = "loads the pinned module and round trips an approval through the host"]
async fn native_fixture_approves_a_parked_request_round_trip() {
    let scratch = tempfile::tempdir().expect("scratch workspace");
    std::env::set_var("OPENHUMAN_WORKSPACE", scratch.path());
    std::env::set_var("OPENHUMAN_KEYRING_BACKEND", "file");
    let mut config = Config::default();
    config.modules.enabled = true;
    config.workspace_dir = scratch.path().to_path_buf();
    config.config_path = scratch.path().join("config.toml");
    config.action_dir = scratch.path().join("action");
    config.action_dir_override = Some(config.action_dir.clone());
    config.save().await.expect("save fixture config");
    openhuman_core::core::auth::init_rpc_token_with_value(TOKEN).expect("install RPC token");
    openhuman_core::security::secret_storage::init_workspace(scratch.path());
    let router = openhuman_rpc::server::build_core_http_router(false);
    client(&config).await.expect("released module loads");
    let gate = ApprovalGate::init_global(config, "session-security-native-approval-fixture");
    let parked = {
        let gate = gate.clone();
        tokio::spawn(async move {
            turn_origin::with_origin(
                AgentTurnOrigin::WebChat {
                    thread_id: "native-fixture-thread".into(),
                    client_id: "native-fixture-client".into(),
                    request_id: None,
                },
                gate.intercept(
                    &ApprovalTurnContext::chat(ApprovalChatContext {
                        thread_id: "native-fixture-thread".into(),
                        client_id: "native-fixture-client".into(),
                        request_id: None,
                    }),
                    "shell",
                    "run harmless fixture command",
                    json!({"command":"echo approval-fixture"}),
                ),
            ).await
        })
    };
    let mut pending = Value::Null;
    for _ in 0..200 {
        pending = rpc(&router, "openhuman.approval_list_pending", json!({})).await;
        if rpc_result(&pending).as_array().is_some_and(|rows| !rows.is_empty()) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let row = rpc_result(&pending).as_array().and_then(|rows| rows.first())
        .expect("TinySecurity should persist the parked approval row");
    let request_id = row["request_id"].as_str().expect("approval request id");
    let decision = rpc(&router, "openhuman.approval_decide", json!({
        "request_id": request_id,
        "decision": "approve_once",
    })).await;
    assert!(decision.get("error").is_none(), "host should record the approval: {decision}");
    assert!(matches!(parked.await.expect("parked caller"), GateOutcome::Allow));
}
