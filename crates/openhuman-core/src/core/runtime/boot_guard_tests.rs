use super::*;

/// A deployment that passes every check: a private root outside any home, a
/// valid token, the SaaS presets, and a clean environment.
struct Fixture {
    _tmp: tempfile::TempDir,
    config: SaasConfig,
    token: ServiceToken,
}

fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("saas");
    std::fs::create_dir(&root).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    Fixture {
        config: SaasConfig::new(root),
        token: ServiceToken::Valid("x".repeat(MIN_SERVICE_TOKEN_LEN)),
        _tmp: tmp,
    }
}

fn inputs<'a>(f: &'a Fixture, env: &'a [(String, String)]) -> BootInputs<'a> {
    BootInputs {
        host_kind: HostKind::Saas,
        services: ServiceSet::saas(),
        domains: DomainSet::saas(),
        config: &f.config,
        token: &f.token,
        env,
        home: Some(PathBuf::from("/home/nobody")),
    }
}

fn violations(inputs: &BootInputs<'_>) -> Vec<Violation> {
    check(inputs)
        .err()
        .map(|e| e.violations)
        .unwrap_or_default()
}

fn env(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn a_clean_deployment_boots() {
    let f = fixture();
    assert_eq!(check(&inputs(&f, &[])), Ok(()));
}

#[test]
fn only_the_saas_host_kind_serves_saas() {
    let f = fixture();
    for kind in [
        HostKind::TauriShell,
        HostKind::Cli,
        HostKind::Docker,
        HostKind::Library,
    ] {
        let mut i = inputs(&f, &[]);
        i.host_kind = kind;
        assert_eq!(violations(&i), vec![Violation::HostKind(kind)]);
    }
}

#[test]
fn background_services_beyond_the_preset_are_refused() {
    let f = fixture();
    let mut i = inputs(&f, &[]);
    i.services = ServiceSet::desktop();
    let found = violations(&i);
    for name in ["socketio", "cron", "channels", "login_gated", "mcp_boot"] {
        assert!(
            found.contains(&Violation::Service(name)),
            "{name}: {found:?}"
        );
    }
    assert!(!found.contains(&Violation::Service("rpc_http")));
}

#[test]
fn domain_families_are_refused_until_they_are_isolated() {
    let f = fixture();
    let mut i = inputs(&f, &[]);
    i.domains = DomainSet::harness();
    let found = violations(&i);
    for name in ["agent", "memory", "config", "security"] {
        assert!(
            found.contains(&Violation::Domain(name)),
            "{name}: {found:?}"
        );
    }
    assert!(
        !found.contains(&Violation::Domain("threads")),
        "threads is isolated per user: {found:?}"
    );
}

#[test]
fn single_user_environment_is_refused() {
    let f = fixture();
    let vars = env(&[
        ("OPENHUMAN_WORKSPACE", "/home/u/.openhuman/users/u1"),
        ("OPENHUMAN_DEV_CONNECT", "1"),
        ("OPENHUMAN_BACKEND_SESSION_TOKEN", "jwt"),
        ("OPENHUMAN_CORE_TOKEN", "t"),
        ("OPENHUMAN_BACKEND_API_KEY", "k"),
    ]);
    let found = violations(&inputs(&f, &vars));
    assert_eq!(found.len(), 5, "{found:?}");
    assert!(found.iter().all(|v| matches!(v, Violation::Env { .. })));
}

#[test]
fn empty_values_do_not_count_as_set() {
    let f = fixture();
    let vars = env(&[("OPENHUMAN_WORKSPACE", "  ")]);
    assert_eq!(check(&inputs(&f, &vars)), Ok(()));
}

#[test]
fn protections_may_be_switched_on_but_not_off() {
    let f = fixture();
    let on = env(&[
        ("OPENHUMAN_APPROVAL_GATE", "1"),
        ("OPENHUMAN_SANDBOX", "on"),
    ]);
    assert_eq!(check(&inputs(&f, &on)), Ok(()));
    let off = env(&[
        ("OPENHUMAN_APPROVAL_GATE", "0"),
        ("OPENHUMAN_SANDBOX", "off"),
    ]);
    assert_eq!(violations(&inputs(&f, &off)).len(), 2);
}

#[test]
fn a_shared_backend_key_needs_the_operator_opt_in() {
    let mut f = fixture();
    let vars = env(&[(SHARED_API_KEY_ENV, "key")]);
    assert_eq!(violations(&inputs(&f, &vars)).len(), 1);
    f.config.shared_backend_api_key = true;
    assert_eq!(check(&inputs(&f, &vars)), Ok(()));
}

#[test]
fn allowlists_are_refused_until_their_isolation_ships() {
    let mut f = fixture();
    f.config.tool_allowlist = vec!["coding".into()];
    f.config.rpc_allowlist_extra = vec!["config.get_config".into()];
    let found = violations(&inputs(&f, &[]));
    assert!(found.contains(&Violation::ToolAllowlist(vec!["coding".into()])));
    assert!(found.contains(&Violation::RpcAllowlist(vec!["config.get_config".into()])));
}

#[test]
fn a_bad_root_is_refused() {
    let mut f = fixture();
    f.config.root = PathBuf::from("relative/root");
    assert!(matches!(
        violations(&inputs(&f, &[]))[..],
        [Violation::Root(_)]
    ));

    f.config.root = PathBuf::from("/definitely/not/here/openhuman-saas");
    assert!(matches!(
        violations(&inputs(&f, &[]))[..],
        [Violation::Root(_)]
    ));
}

#[test]
fn a_root_inside_the_desktop_install_is_refused() {
    let f = fixture();
    let mut i = inputs(&f, &[]);
    // Treat the fixture's parent as the home directory and nest the root in
    // its `.openhuman`.
    let home = f.config.root.parent().unwrap().to_path_buf();
    let nested = home.join(".openhuman").join("saas");
    std::fs::create_dir_all(&nested).unwrap();
    let config = SaasConfig::new(nested);
    i.config = &config;
    i.home = Some(home);
    let found = violations(&i);
    assert!(
        matches!(&found[..], [Violation::Root(why)] if why.contains(".openhuman")),
        "{found:?}"
    );
}

#[cfg(unix)]
#[test]
fn a_world_writable_root_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let f = fixture();
    std::fs::set_permissions(&f.config.root, std::fs::Permissions::from_mode(0o777)).unwrap();
    let found = violations(&inputs(&f, &[]));
    assert!(
        matches!(&found[..], [Violation::Root(why)] if why.contains("world-writable")),
        "{found:?}"
    );
}

#[test]
fn an_invalid_token_is_refused() {
    let mut f = fixture();
    f.token = ServiceToken::Invalid("missing".into());
    assert_eq!(
        violations(&inputs(&f, &[])),
        vec![Violation::ServiceToken("missing".into())]
    );
}

#[test]
fn every_violation_is_reported_at_once() {
    let mut f = fixture();
    f.token = ServiceToken::Invalid("missing".into());
    f.config.tool_allowlist = vec!["coding".into()];
    let vars = env(&[("OPENHUMAN_WORKSPACE", "/w")]);
    let mut i = inputs(&f, &vars);
    i.host_kind = HostKind::Cli;
    let err = check(&i).unwrap_err();
    assert_eq!(err.violations.len(), 4, "{err}");
    let text = err.to_string();
    assert!(text.contains("4 problem(s)"), "{text}");
}

#[test]
fn service_token_file_rules() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("service.token");

    assert!(matches!(
        ServiceToken::read(&path),
        ServiceToken::Invalid(_)
    ));

    std::fs::write(&path, "short\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    assert!(
        matches!(ServiceToken::read(&path), ServiceToken::Invalid(why) if why.contains("at least"))
    );

    let bearer = "b".repeat(MIN_SERVICE_TOKEN_LEN + 4);
    std::fs::write(&path, format!("{bearer}\n")).unwrap();
    assert_eq!(ServiceToken::read(&path), ServiceToken::Valid(bearer));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(
            matches!(ServiceToken::read(&path), ServiceToken::Invalid(why) if why.contains("readable by others"))
        );
    }
}
