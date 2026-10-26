use super::*;

#[test]
fn keyring_callback_fails_closed_without_operation_scoped_approval() {
    use tinysecurity_bus::KeyringConsentOutcome2 as Outcome;

    let request = tinysecurity_bus::KeyringConsentRequest2 {
        caller: Default::default(),
        purpose: "credential profile access".into(),
        access: tinysecurity_bus::KeyringAccess2::Read,
        name: "auth:provider:default".into(),
        timeout_ms: 30_000,
    };
    assert_eq!(keyring_consent_outcome(&request), Outcome::Unavailable);
}

#[test]
fn unmigrated_callbacks_return_an_error_instead_of_authority() {
    assert!(
        unavailable()
            .to_string()
            .contains("security host callback unavailable")
    );
}
