use openhuman_core::{
    config::Config,
    core::runtime::{
        turn_origin::{self, AgentTurnOrigin},
        DispatchCtx,
    },
    modules::security::{client, reinitialize},
    sandbox::types::{SandboxBackendKind, SandboxPolicy},
    security::approval::{
        gate::ApprovalGate, ApprovalChatContext, ApprovalTurnContext, GateOutcome,
    },
};
use openhuman_rpc::{
    embed::ServiceSet,
    host::{desktop_builder, serve_desktop, DesktopOptions, EmbeddedReadySignal},
};
use serde_json::{json, Value};
use std::path::PathBuf;
use tinysecurity_bus::{
    CommandAutonomy, CommandClass, CommandExecutionMode, CommandPolicy, CommandRequest,
    ProtectedPathRequest2, RedactMode,
};
use tokio_util::sync::CancellationToken;

const TOKEN: &str = "security-native-fixture-token";

fn run_on_agent_stack<F, Fut>(name: &str, future_factory: F)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    std::thread::Builder::new()
        .name(name.to_owned())
        .stack_size(openhuman_core::core::runtime::AGENT_WORKER_STACK_BYTES)
        .spawn(move || {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_stack_size(openhuman_core::core::runtime::AGENT_WORKER_STACK_BYTES)
                .enable_all()
                .build()
                .expect("build native fixture runtime")
                .block_on(future_factory());
        })
        .expect("spawn native fixture runtime")
        .join()
        .expect("native fixture should not panic");
}

async fn rpc_http(port: u16, method: &str, params: Value) -> Value {
    let response = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{port}/rpc"))
        .bearer_auth(TOKEN)
        .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
        .send()
        .await
        .expect("RPC request");
    assert_eq!(response.status(), reqwest::StatusCode::OK, "{method}");
    response.json().await.expect("JSON-RPC response")
}

async fn start_desktop_fixture(
    config: Config,
) -> (
    u16,
    CancellationToken,
    tokio::task::JoinHandle<anyhow::Result<()>>,
) {
    openhuman_rpc::host::prepare_cli_host();
    let options = DesktopOptions {
        host: Some("127.0.0.1".into()),
        port: Some(0),
        socketio: false,
        rpc_token: Some(std::sync::Arc::new(TOKEN.into())),
    };
    let services = ServiceSet {
        rpc_http: true,
        socketio: false,
        cron: false,
        channels: false,
        login_gated: false,
        update_scheduler: false,
        memory_queue: false,
        skill_catalog_refresh: false,
        mcp_boot: false,
        integrations: false,
        memory_sync: false,
    };
    let builder = desktop_builder(&options).config(config).services(services);
    let shutdown = CancellationToken::new();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<EmbeddedReadySignal>();
    let server = tokio::spawn(serve_desktop(builder, shutdown.clone(), ready_tx));
    let ready = tokio::time::timeout(std::time::Duration::from_secs(120), ready_rx)
        .await
        .expect("desktop host ready signal")
        .expect("desktop host ready channel");
    (ready.port, shutdown, server)
}

fn rpc_result(response: &Value) -> &Value {
    assert!(response.get("error").is_none(), "{response}");
    let result = &response["result"];
    if result.get("logs").is_some() {
        &result["result"]
    } else {
        result
    }
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

    // Browser navigation uses this production URL guard before DNS/connect;
    // exercise loopback and the cloud metadata address without network access.
    assert!(tinyagents_harness::url_guard::is_non_global_v4(
        std::net::Ipv4Addr::LOCALHOST
    ));
    assert!(tinyagents_harness::url_guard::is_non_global_v4(
        std::net::Ipv4Addr::new(169, 254, 169, 254)
    ));

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
    assert!(
        keyring.is_err(),
        "keyring access without host consent must fail closed"
    );
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

/// An unbound caller cannot ask TinySecurity to access the OS keyring; the
/// module must fail closed before any platform backend is touched.
#[tokio::test]
#[ignore = "loads the pinned module and verifies keyring access fails closed without host admission"]
async fn native_fixture_denies_keyring_before_backend_access() {
    let scratch = tempfile::tempdir().expect("scratch workspace");
    std::env::set_var("OPENHUMAN_WORKSPACE", scratch.path());
    std::env::set_var("OPENHUMAN_KEYRING_BACKEND", "file");
    let mut config = Config::default();
    config.modules.enabled = true;
    config.workspace_dir = scratch.path().to_path_buf();
    config.config_path = scratch.path().join("config.toml");
    config.action_dir = scratch.path().join("action");
    config.action_dir_override = Some(config.action_dir.clone());
    config.save().await.expect("save keyring fixture config");
    openhuman_core::security::secret_storage::init_workspace(scratch.path());
    let client = client(&config).await.expect("released module loads");
    let result = client
        .keyring_access2(tinysecurity_bus::KeyringRequest2 {
            caller: tinysecurity_bus::CallerContext {
                agent: "native-fixture".into(),
                call_id: "native-fixture-keyring-call".into(),
                ..Default::default()
            },
            purpose: "verify fail-closed keyring access".into(),
            operation: tinysecurity_bus::KeyringOperation2::Get {
                name: "fixture-secret".into(),
            },
        })
        .await;
    assert!(
        result.is_err(),
        "keyring access without host admission must fail closed"
    );
}

/// Runs the public shell adapter through the production desktop host so the
/// released module receives its registered sandbox and audit callbacks.
#[test]
#[ignore = "loads the pinned module and exercises its sandbox plan/consume path"]
fn native_fixture_executes_through_the_module_sandbox_plan() {
    run_on_agent_stack("native-sandbox-fixture", || async {
        let scratch = tempfile::tempdir().expect("scratch workspace");
        std::env::set_var("OPENHUMAN_WORKSPACE", scratch.path());
        std::env::set_var("OPENHUMAN_ACTION_DIR", scratch.path());
        std::env::set_var("OPENHUMAN_SANDBOX", "off");
        let mut config = Config::default();
        config.modules.enabled = true;
        config.workspace_dir = scratch.path().to_path_buf();
        config.config_path = scratch.path().join("config.toml");
        config.action_dir = scratch.path().to_path_buf();
        config.action_dir_override = Some(config.action_dir.clone());
        config.save().await.expect("save sandbox fixture config");
        let (_port, shutdown, server) = start_desktop_fixture(config.clone()).await;
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
        let command = "echo sandbox-fixture";
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
        .await;
        let result = result.expect("TinySecurity should plan and consume the safe sandbox command");
        assert!(
            result.success(),
            "sandbox command failed: {}",
            result.stderr
        );
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
        assert!(
            denied.is_err(),
            "sandbox path policy must refuse cwd outside its workspace"
        );
        shutdown.cancel();
        server
            .await
            .expect("desktop server task")
            .expect("desktop server shuts down cleanly");
    });
}

/// Parks a real approval request, approves it through the host RPC controller,
/// and checks that the caller resumes with an allow outcome.
#[test]
#[ignore = "loads the pinned module and round trips an approval through the host"]
fn native_fixture_approves_a_parked_request_round_trip() {
    run_on_agent_stack("native-approval-fixture", || async {
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
        let (port, shutdown, server) = start_desktop_fixture(config.clone()).await;
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
                )
                .await
            })
        };
        let mut pending = Value::Null;
        for _ in 0..200 {
            pending = rpc_http(port, "openhuman.approval_list_pending", json!({})).await;
            if rpc_result(&pending)
                .as_array()
                .is_some_and(|rows| !rows.is_empty())
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        let row = rpc_result(&pending)
            .as_array()
            .and_then(|rows| rows.first())
            .unwrap_or_else(|| {
                panic!(
                "TinySecurity should persist the parked approval row: {pending}; task finished: {}",
                parked.is_finished()
            )
            });
        let request_id = row["request_id"].as_str().expect("approval request id");
        let decision = rpc_http(
            port,
            "openhuman.approval_decide",
            json!({
                "request_id": request_id,
                "decision": "approve_once",
            }),
        )
        .await;
        assert!(
            decision.get("error").is_none(),
            "host should record the approval: {decision}"
        );
        assert!(matches!(
            parked.await.expect("parked caller"),
            GateOutcome::Allow
        ));
        shutdown.cancel();
        server
            .await
            .expect("desktop server task")
            .expect("desktop server shuts down cleanly");
    });
}
