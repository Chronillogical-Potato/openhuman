use super::approval::approval_is_explicitly_disabled;
use crate::security::approval::gate::ApprovalGateBootState;

#[test]
fn missing_gate_is_allowed_only_after_explicit_operator_disable() {
    let installed = ApprovalGateBootState {
        installed: true,
        disabled_by_env: false,
        override_ignored: false,
        host: "cli",
    };
    let disabled = ApprovalGateBootState {
        installed: false,
        disabled_by_env: true,
        override_ignored: false,
        host: "cli",
    };

    assert!(!approval_is_explicitly_disabled(None));
    assert!(!approval_is_explicitly_disabled(Some(installed)));
    assert!(approval_is_explicitly_disabled(Some(disabled)));
}
