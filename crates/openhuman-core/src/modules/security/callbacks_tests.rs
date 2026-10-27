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
    assert!(unavailable()
        .to_string()
        .contains("security host callback unavailable"));
}

#[tokio::test]
async fn pairing_callbacks_fail_closed_without_an_admitted_host_event() {
    let host = DevicePairingHost;
    let result = host
        .resolve_pairing_event(tinysecurity_bus::DevicePairingContextRequest2 {
            host_event_id: "unissued-event".into(),
            purpose: tinysecurity_bus::DevicePairingPurpose2::UserController,
            requested_channel_id: None,
        })
        .await;
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("security host callback unavailable"));
}

#[tokio::test]
async fn pairing_persistence_callbacks_fail_closed_without_a_bound_scope() {
    let host = DevicePairingHost;
    let result = host
        .persist_device_pairing(tinysecurity_bus::DevicePairingPersistenceRequest2 {
            backend_id: "unconfigured".into(),
            scope_id: "unconfigured".into(),
            secret_registration_id: "unconfigured".into(),
            operation: tinysecurity_bus::DevicePairingPersistenceOperation2::Load,
        })
        .await;
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("security host callback unavailable"));
}

#[tokio::test]
async fn legacy_pairing_import_fails_closed_without_a_one_shot_authority() {
    let host = DevicePairingHost;
    let result = host
        .load_legacy_device_identities(tinysecurity_bus::DevicePairingLegacyLoadRequest2 {
            handle: "unissued-import".into(),
        })
        .await;
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("security host callback unavailable"));
}
