use super::*;
use serde_json::json;

#[test]
fn invocation_result_ok_serializes_value() {
    let result = InvocationResult::ok(json!({"key": "value"})).unwrap();
    assert_eq!(result.value, json!({"key": "value"}));
    assert!(result.logs.is_empty());
}

#[test]
fn invocation_to_rpc_json_no_logs_returns_value_directly() {
    let inv = InvocationResult {
        value: json!({"data": true}),
        logs: vec![],
    };
    let json = invocation_to_rpc_json(inv);
    assert_eq!(json, json!({"data": true}));
}

#[test]
fn invocation_to_rpc_json_with_logs_wraps_in_envelope() {
    let inv = InvocationResult {
        value: json!({"data": true}),
        logs: vec!["info".into()],
    };
    let json = invocation_to_rpc_json(inv);
    assert!(json.get("result").is_some());
    assert!(json.get("logs").is_some());
    assert_eq!(json["result"], json!({"data": true}));
    assert_eq!(json["logs"][0], "info");
}

/// The two controller return paths must produce byte-identical JSON for the
/// same (value, logs) pair.
///
/// `Outcome::into_cli_compatible_json` (the registry path, 152 call
/// sites) and `invocation_to_rpc_json` (the dynamic-dispatch path) used to
/// carry independent copies of the same envelope rule. Nothing linked them,
/// so a fix or a normalisation applied to one would have left the other
/// answering the old shape — the divergence would have been invisible until
/// a caller hit the wrong path. Both now delegate to
/// `rpc::apply_log_envelope`; this asserts they still agree, so a future
/// edit cannot re-fork them without failing here.
///
/// It deliberately asserts *agreement*, not a particular shape: the shape
/// itself is the open question in #6080, and pinning it here would enshrine
/// the defect as intended behaviour.
#[test]
fn both_controller_return_paths_apply_the_same_log_envelope() {
    for (value, logs) in [
        (json!({ "data": true }), Vec::<String>::new()),
        (json!({ "data": true }), vec!["one".to_string()]),
        (json!(null), vec!["log on a null value".to_string()]),
        (json!([1, 2, 3]), Vec::<String>::new()),
        // A value that already carries a `result` key: the envelope must
        // treat it as opaque data, not as an envelope to flatten.
        (json!({ "result": "inner", "logs": ["not mine"] }), vec![]),
    ] {
        let via_dispatch = invocation_to_rpc_json(InvocationResult {
            value: value.clone(),
            logs: logs.clone(),
        });
        let via_registry = crate::core::Outcome::new(value.clone(), logs.clone())
            .into_cli_compatible_json()
            .expect("a serde_json::Value always serializes");

        assert_eq!(
            via_dispatch, via_registry,
            "the dispatch and registry paths disagreed for value={value} logs={logs:?}; \
                 both must go through rpc::apply_log_envelope"
        );
    }
}

#[test]
fn host_kind_tag_is_stable() {
    // Downstream consumers (event-bus subscribers, log shippers) key
    // on the exact tag strings; pin them so a rename is loud.
    assert_eq!(HostKind::TauriShell.tag(), "tauri-shell");
    assert_eq!(HostKind::Cli.tag(), "cli");
    assert_eq!(HostKind::Docker.tag(), "docker");
    assert_eq!(HostKind::Library.tag(), "library");
}

#[test]
fn host_kind_is_desktop_shell_only_for_tauri() {
    assert!(HostKind::TauriShell.is_desktop_shell());
    assert!(!HostKind::Cli.is_desktop_shell());
    assert!(!HostKind::Docker.is_desktop_shell());
    assert!(!HostKind::Library.is_desktop_shell());
}

#[test]
fn desktop_shell_ignores_env_override() {
    // Operator sets OPENHUMAN_APPROVAL_GATE=0 inside a Tauri-shell
    // boot — the gate MUST still install, and the override-ignored
    // signal MUST fire so the UI can banner.
    let d = approval_gate_boot_decision(HostKind::TauriShell, true);
    assert!(d.install_gate, "tauri shell must always install the gate");
    assert!(
        d.override_ignored,
        "tauri shell must surface that an override was attempted + ignored"
    );
    assert!(
        !d.gate_disabled_by_override,
        "desktop path never reports the gate as disabled by env"
    );
}

#[test]
fn desktop_shell_with_no_override_keeps_gate_silent() {
    // Normal Tauri boot, no env override — gate installs, no banners,
    // no warning event. This is the steady-state desktop path.
    let d = approval_gate_boot_decision(HostKind::TauriShell, false);
    assert!(d.install_gate);
    assert!(!d.override_ignored);
    assert!(!d.gate_disabled_by_override);
}

#[test]
fn standalone_cli_honors_env_override_with_warning_signal() {
    let d = approval_gate_boot_decision(HostKind::Cli, true);
    assert!(!d.install_gate, "CLI must honor the operator env override");
    assert!(
        d.gate_disabled_by_override,
        "CLI must surface the elevated-privilege state via the disabled event"
    );
    assert!(
        !d.override_ignored,
        "CLI doesn't ignore; it honors — only desktop ignores"
    );
}

#[test]
fn standalone_docker_honors_env_override_with_warning_signal() {
    let d = approval_gate_boot_decision(HostKind::Docker, true);
    assert!(!d.install_gate);
    assert!(d.gate_disabled_by_override);
    assert!(!d.override_ignored);
}

#[test]
fn standalone_with_no_env_override_installs_gate_silently() {
    for host in [HostKind::Cli, HostKind::Docker, HostKind::Library] {
        let d = approval_gate_boot_decision(host, false);
        assert!(
            d.install_gate,
            "{host:?} with no override must install the gate"
        );
        assert!(!d.override_ignored);
        assert!(!d.gate_disabled_by_override);
    }
}
