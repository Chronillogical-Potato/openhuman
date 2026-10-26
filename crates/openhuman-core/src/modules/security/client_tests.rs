use super::*;

#[test]
fn keyring_access_preserves_exact_tinysecurity_consent_errors() {
    let denied = tinybus::module::ModuleCallFailure {
        kind: tinybus::module::ModuleCallError::ModuleFault,
        wire_name: "ai.tinyhumans.tinysecurity.Error.ConsentDenied".into(),
        message: "keyring consent outcome".into(),
    };
    assert_eq!(keyring_access_error(denied), SecurityError::ConsentDenied);

    let pending = tinybus::module::ModuleCallFailure {
        kind: tinybus::module::ModuleCallError::ModuleFault,
        wire_name: "ai.tinyhumans.tinysecurity.Error.ConsentPending".into(),
        message: "keyring consent outcome".into(),
    };
    assert_eq!(keyring_access_error(pending), SecurityError::ConsentPending);
}
