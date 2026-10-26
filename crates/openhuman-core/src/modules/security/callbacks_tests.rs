use super::*;

#[test]
fn keyring_callback_grants_only_explicit_operation_scoped_approval() {
    use tinysecurity_bus::KeyringConsentOutcome2 as Outcome;
    assert_eq!(keyring_consent_outcome(Some(true)), Outcome::Allowed);
    assert_eq!(keyring_consent_outcome(Some(false)), Outcome::ConsentDenied);
    assert_eq!(keyring_consent_outcome(None), Outcome::Unavailable);
}

#[test]
fn unmigrated_callbacks_return_an_error_instead_of_authority() {
    assert!(
        unavailable()
            .to_string()
            .contains("security host callback unavailable")
    );
}
