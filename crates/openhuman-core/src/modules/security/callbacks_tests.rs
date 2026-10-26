use super::*;

#[test]
fn keyring_callback_uses_the_existing_consent_decision() {
    use crate::security::keyring_consent::PolicyDecision;
    use tinysecurity_bus::KeyringConsentOutcome2 as Outcome;

    assert_eq!(
        keyring_consent_outcome(PolicyDecision::Proceed),
        Outcome::Allowed
    );
    assert_eq!(
        keyring_consent_outcome(PolicyDecision::ConsentRequired),
        Outcome::ConsentPending
    );
    assert_eq!(
        keyring_consent_outcome(PolicyDecision::Declined),
        Outcome::ConsentDenied
    );
}

#[test]
fn unmigrated_callbacks_return_an_error_instead_of_authority() {
    assert!(
        unavailable()
            .to_string()
            .contains("security host callback unavailable")
    );
}
