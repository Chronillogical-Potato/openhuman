use axum::{body::{to_bytes, Body}, http::{Request, StatusCode}, Router};
use openhuman_core::{config::Config, modules::security::{client, reinitialize}};
use std::path::PathBuf;
use tinysecurity_bus::{
    HostActionClassRequest2, PathAccess, PathBatchRequest2, PathCheckKind, PathPolicy,
    PathTrustedRoot, PrivacyCheckRequest2, PrivacyDataClass2, PrivacyMode,
    ProtectedPathRequest2, RedactMode, ToolPermission, Verdict,
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

/// Loads the published TinySecurity archive through the compiled registry pin.
/// This stays as a separate integration target so its focused native check
/// does not compile unrelated core unit-test modules.
#[tokio::test]
#[ignore = "downloads and loads the pinned native TinySecurity release"]
async fn native_fixture_exercises_the_pinned_release_contracts() {
    let mut config = Config::default();
    config.modules.enabled = true;

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

    let workspace = std::env::current_dir().expect("fixture has a current directory");
    let path_scope = client
        .register_path_policy(PathPolicy {
            enabled: true,
            workspace_only: true,
            workspace_dir: workspace.clone(),
            action_dir: workspace.clone(),
            home_dir: None,
            forbidden_paths: vec![],
            trusted_roots: vec![PathTrustedRoot {
                path: workspace,
                access: PathAccess::ReadWrite,
            }],
            reserved_paths: vec![],
            reserved_prefixes: vec![],
            readonly_paths: vec![],
            reserved_names: vec![],
        })
        .await
        .expect("the host should be able to register an immutable path scope");
    let path_result = client
        .check_paths2(PathBatchRequest2 {
            policy_id: path_scope,
            kind: PathCheckKind::StringAllowed,
            paths: vec![PathBuf::from("../outside"), PathBuf::from("ordinary.txt")],
        })
        .await
        .expect("the scoped path check should answer");
    assert_eq!(path_result.allowed, vec![false, true]);

    let class = client
        .classify_host_action2(HostActionClassRequest2 {
            permission: ToolPermission::Execute,
            external_effect: false,
        })
        .await
        .expect("shell action classification should answer");
    assert_eq!(class.class, tinysecurity_bus::CommandClass::Write);

    let egress = client
        .privacy_check2(PrivacyCheckRequest2 {
            privacy: PrivacyMode::LocalOnly,
            external: true,
            class: PrivacyDataClass2::UserContent,
            destination_label: "test-destination".to_owned(),
            risk: None,
        })
        .await
        .expect("the privacy/egress policy should answer");
    assert!(matches!(egress.verdict, Verdict::Deny { .. }));

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
