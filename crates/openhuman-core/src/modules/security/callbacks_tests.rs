use super::*;

fn pairing_context(
    channel_id: &str,
    source: tinysecurity_bus::DevicePairingSource2,
) -> tinysecurity_bus::DevicePairingContextResponse2 {
    tinysecurity_bus::DevicePairingContextResponse2 {
        scope_id: "0123456789abcdef0123456789abcdef".into(),
        user_binding: "opaque-user-binding".into(),
        secret_master_handle: Some("master-handle".into()),
        legacy_import_handle: None,
        channel_id: channel_id.into(),
        pairing_token: (!channel_id.is_empty()).then(|| "pairing-token".into()),
        connection_nonce: (!channel_id.is_empty())
            .then(|| "connection-nonce".into())
            .unwrap_or_default(),
        generation: 7,
        source,
        expires_at_ms: u64::MAX,
    }
}

#[test]
fn device_pairing_host_event_is_bound_to_purpose_channel_and_used_once() {
    use tinysecurity_bus::{DevicePairingPurpose2, DevicePairingSource2};

    let events = PairingEventRegistry::default();
    let event_id = events.issue(pairing_context(
        "channel-1",
        DevicePairingSource2::RegisteredSocket,
    ));
    let request = tinysecurity_bus::DevicePairingContextRequest2 {
        host_event_id: event_id.clone(),
        purpose: DevicePairingPurpose2::SocketEvent,
        requested_channel_id: Some("channel-1".into()),
    };

    assert_eq!(
        events.resolve(&request, 1_000).unwrap().channel_id,
        "channel-1"
    );
    assert!(
        events.resolve(&request, 1_000).is_err(),
        "event IDs are single use"
    );

    let wrong_purpose = events.issue(pairing_context(
        "channel-2",
        DevicePairingSource2::RegisteredSocket,
    ));
    assert!(events
        .resolve(
            &tinysecurity_bus::DevicePairingContextRequest2 {
                host_event_id: wrong_purpose,
                purpose: DevicePairingPurpose2::Reconnect,
                requested_channel_id: Some("channel-2".into()),
            },
            1_000,
        )
        .is_err());

    let wrong_channel = events.issue(pairing_context(
        "channel-3",
        DevicePairingSource2::RegisteredSocket,
    ));
    assert!(events
        .resolve(
            &tinysecurity_bus::DevicePairingContextRequest2 {
                host_event_id: wrong_channel,
                purpose: DevicePairingPurpose2::SocketEvent,
                requested_channel_id: Some("attacker-channel".into()),
            },
            1_000,
        )
        .is_err());
}

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
