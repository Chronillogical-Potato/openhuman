use super::*;

#[test]
fn keyring_access_preserves_exact_tinysecurity_consent_errors() {
    let denied = tinybus::Error::MethodFailed {
        name: "ai.tinyhumans.tinysecurity.Error.ConsentDenied".into(),
        message: "keyring consent outcome".into(),
    };
    assert_eq!(keyring_access_error(denied), SecurityError::ConsentDenied);

    let pending = tinybus::Error::MethodFailed {
        name: "ai.tinyhumans.tinysecurity.Error.ConsentPending".into(),
        message: "keyring consent outcome".into(),
    };
    assert_eq!(keyring_access_error(pending), SecurityError::ConsentPending);
}
