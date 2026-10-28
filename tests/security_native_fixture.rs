use openhuman_core::{
    config::Config,
    modules::security::{client, reinitialize},
};
use std::path::PathBuf;
use tinysecurity_bus::{
    HostActionClassRequest2, PathAccess, PathBatchRequest2, PathCheckKind, PathPolicy,
    PathTrustedRoot, PrivacyCheckRequest2, PrivacyDataClass2, PrivacyMode,
    ProtectedPathRequest2, RedactMode, ToolPermission,
};

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
            kind: PathCheckKind::Protected,
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
    assert_eq!(egress, tinysecurity_bus::Decision::Deny);

    let keyring = client
        .keyring_access2(tinysecurity_bus::KeyringRequest2 {
            caller: tinysecurity_bus::CallerContext {
                user: "fixture-user".to_owned(),
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
