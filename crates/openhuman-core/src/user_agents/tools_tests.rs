use super::*;
use std::path::PathBuf;

#[test]
fn groups_parse_by_id() {
    for group in SaasToolGroup::ALL {
        assert_eq!(SaasToolGroup::parse(group.id()), Some(group));
    }
    assert_eq!(
        SaasToolGroup::parse(" host_files "),
        Some(SaasToolGroup::HostFiles)
    );
    assert_eq!(SaasToolGroup::parse("coding"), None);
}

#[test]
fn only_the_shell_needs_the_container() {
    assert!(SaasToolGroup::HostShell.needs_sandbox());
    assert!(!SaasToolGroup::HostFiles.needs_sandbox());
}

#[test]
fn no_group_opens_a_hard_denied_tool() {
    for group in SaasToolGroup::ALL {
        for tool in group.tools() {
            assert!(
                !is_hard_denied(tool),
                "{} opens denied `{tool}`",
                group.id()
            );
        }
    }
}

#[test]
fn allowlist_parsing_keeps_groups_and_reports_the_rest() {
    let entries = vec!["host_shell".to_string(), "coding".to_string()];
    assert_eq!(parse_allowlist(&entries), vec![SaasToolGroup::HostShell]);
    assert_eq!(unknown_entries(&entries), vec!["coding".to_string()]);
}

#[test]
fn host_tools_need_their_group() {
    assert!(!admits_with("shell", true, &[]));
    assert!(!admits_with("file_read", true, &[SaasToolGroup::HostShell]));
    assert!(admits_with("shell", false, &[SaasToolGroup::HostShell]));
    assert!(admits_with("edit", false, &[SaasToolGroup::HostFiles]));
}

#[test]
fn other_tools_follow_their_domain() {
    assert!(admits_with("memory_recall", true, &[]));
    assert!(!admits_with("memory_recall", false, &SaasToolGroup::ALL));
}

#[test]
fn hard_denied_tools_never_pass() {
    for tool in [
        "install_tool",
        "service_restart",
        "mcp_registry_add",
        "browser_open",
        "curl",
    ] {
        assert!(!admits_with(tool, true, &SaasToolGroup::ALL), "{tool}");
    }
}

#[test]
fn the_gate_allows_only_allowlisted_groups() {
    assert!(gate_verdict_with("shell", &[SaasToolGroup::HostShell]).is_ok());
    let refused = gate_verdict_with("shell", &[]).unwrap_err();
    assert!(refused.contains("no approval surface"), "{refused}");
    // A domain tool that asks for approval has nowhere to ask in SaaS.
    assert!(gate_verdict_with("send_email", &SaasToolGroup::ALL).is_err());
}

#[test]
fn outside_saas_the_filter_is_the_domain_filter() {
    assert!(admits("shell", true));
    assert!(!admits("memory_recall", false));
}

fn root() -> PathBuf {
    PathBuf::from("/srv/oh")
}

#[test]
fn the_sandbox_policy_is_a_locked_down_container() {
    let action = root().join("agents/u-abc/sandbox");
    let state = root().join("agents/u-abc/workspace");
    let policy =
        sandbox_policy_with(&root(), &SaasSandboxConfig::default(), &action, &state).unwrap();
    assert_eq!(policy.backend, SandboxBackendKind::Docker);
    assert_eq!(policy.workspace_root, action);
    assert!(!policy.allow_network);
    assert!(policy.env_passthrough.is_empty());
    assert!(policy.read_only_mounts.is_empty() && policy.read_write_mounts.is_empty());
    let docker = policy.docker_overrides.unwrap();
    assert_eq!(docker.read_only_rootfs, Some(true));
    assert_eq!(docker.network.as_deref(), Some("none"));
}

#[test]
fn a_named_network_allows_egress() {
    let config = SaasSandboxConfig {
        network: "egress".into(),
        ..SaasSandboxConfig::default()
    };
    let action = root().join("agents/u-abc/sandbox");
    let policy = sandbox_policy_with(&root(), &config, &action, &action).unwrap();
    assert!(policy.allow_network);
}

#[test]
fn the_sandbox_refuses_anything_but_a_user_sandbox() {
    let config = SaasSandboxConfig::default();
    for dir in [
        "/srv/oh/agents/u-abc/workspace",
        "/srv/oh/operator/sandbox",
        "/srv/oh/agents/sandbox",
        "/tmp/sandbox",
        "/srv/oh/agents/u-abc/sandbox/nested",
        "/srv/oh/agents/../sandbox",
        "/srv/oh/agents/not-an-agent/sandbox",
    ] {
        let dir = PathBuf::from(dir);
        assert!(
            sandbox_policy_with(&root(), &config, &dir, &dir).is_err(),
            "{}",
            dir.display()
        );
    }
}

#[test]
fn the_host_network_is_refused() {
    let config = SaasSandboxConfig {
        network: "host".into(),
        ..SaasSandboxConfig::default()
    };
    let action = root().join("agents/u-abc/sandbox");
    assert!(sandbox_policy_with(&root(), &config, &action, &action).is_err());
}
