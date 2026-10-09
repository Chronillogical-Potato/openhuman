use super::*;

#[test]
fn loads_an_operator_file_with_defaults() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("operator.toml");
    std::fs::write(&path, "root = \"/srv/openhuman\"\n").unwrap();
    let config = SaasConfig::load(&path).unwrap();
    assert_eq!(config, SaasConfig::new("/srv/openhuman"));
    assert_eq!(
        config.service_token_path(),
        PathBuf::from("/srv/openhuman/service.token")
    );
}

#[test]
fn reads_every_operator_setting() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("operator.toml");
    std::fs::write(
        &path,
        r#"
root = "/srv/oh"
service_token_file = "/run/secrets/gateway"
tool_allowlist = ["documents"]
rpc_allowlist_extra = ["threads.list"]
max_agents_open = 8
idle_evict_secs = 60
shared_backend_api_key = true
custom_definitions = true
require_user_signature = false
"#,
    )
    .unwrap();
    let config = SaasConfig::load(&path).unwrap();
    assert_eq!(
        config.service_token_path(),
        PathBuf::from("/run/secrets/gateway")
    );
    assert_eq!(config.tool_allowlist, vec!["documents".to_string()]);
    assert_eq!(config.rpc_allowlist_extra, vec!["threads.list".to_string()]);
    assert_eq!(config.max_agents_open, 8);
    assert_eq!(config.idle_evict_secs, 60);
    assert!(config.shared_backend_api_key && config.custom_definitions);
    assert!(!config.require_user_signature);
}

#[test]
fn unknown_keys_are_refused() {
    // A misspelt safety setting must not be silently ignored.
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("operator.toml");
    std::fs::write(
        &path,
        "root = \"/srv/oh\"\ntool_allow_list = [\"coding\"]\n",
    )
    .unwrap();
    let err = SaasConfig::load(&path).unwrap_err().to_string();
    assert!(err.contains("tool_allow_list"), "{err}");
}

#[test]
fn a_missing_file_names_the_path() {
    let err = SaasConfig::load(Path::new("/nope/operator.toml"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("/nope/operator.toml"), "{err}");
}

#[test]
fn the_operator_plane_lives_under_the_root() {
    let config = SaasConfig::new("/srv/oh");
    let operator = config.operator_config();
    for path in [
        &operator.workspace_dir,
        &operator.config_path,
        &operator.action_dir,
    ] {
        assert!(path.starts_with("/srv/oh/operator"), "{}", path.display());
    }
}

#[test]
fn the_saas_presets_are_closed() {
    assert_eq!(
        DomainSet::saas(),
        DomainSet {
            operator: true,
            ..DomainSet::none()
        },
        "only the operator plane until user families are isolated"
    );
    let services = ServiceSet::saas();
    assert!(services.rpc_http);
    assert_eq!(
        ServiceSet {
            rpc_http: false,
            ..services
        },
        ServiceSet::none()
    );
}
