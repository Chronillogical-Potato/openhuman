use super::*;
use crate::security::devices::types::PairingSession;

fn session(expires_at: &str) -> PairingSession {
    PairingSession {
        channel_id: "bus-test-channel".into(),
        pairing_token: "pairing-token".into(),
        core_pubkey: "module-public-key".into(),
        pairing_receipt: "owner-receipt".into(),
        label: "phone".into(),
        rpc_url: None,
        expires_at: expires_at.into(),
        agent: None,
    }
}

fn paired_device() -> super::super::types::PairedDevice {
    super::super::types::PairedDevice {
        channel_id: "bus-test-channel".into(),
        label: "phone".into(),
        device_pubkey: "device-static-key".into(),
        created_at: "2026-01-01T00:00:00Z".into(),
        last_seen_at: None,
        peer_online: None,
        revoked: false,
        pairing_id: Some("module-pairing-id".into()),
        core_private_key_encrypted: None,
    }
}

#[test]
fn socket_frame_encoding_round_trips_without_core_crypto() {
    let frame = [0x02, 1, 2, 3, 0xff];
    let encoded = encode_frame(&frame);
    assert_eq!(decode_frame(&encoded).unwrap(), frame);
    assert!(decode_frame("not-base64url!").is_none());
}

#[test]
fn pairing_session_expiry_is_checked_before_handshake_dispatch() {
    assert!(!pairing_session_is_live(&session("2020-01-01T00:00:00Z")));
    assert!(pairing_session_is_live(&session("2099-01-01T00:00:00Z")));
    assert!(!pairing_session_is_live(&session("not-an-expiry")));
}

#[test]
fn process_restart_unlock_then_peer_online_renews_only_the_same_device() {
    let device = paired_device();
    // Process restart cleared both volatile capabilities. A live persisted
    // row alone cannot authorize renewal until a direct user Unlock succeeds.
    assert!(renewal_pairing_id(false, false, false, false, &device).is_none());
    assert!(renewal_pairing_id(true, false, false, false, &device).is_none());

    // After the direct user path unlocks TinySecurity and the authenticated
    // socket reports this channel online, Renew targets its persisted owner id.
    assert_eq!(
        renewal_pairing_id(true, true, false, false, &device),
        Some("module-pairing-id")
    );
    assert!(renewal_device_matches(
        &device,
        "module-pairing-id",
        "module-pairing-id",
        "device-static-key"
    ));
    assert!(!renewal_device_matches(
        &device,
        "module-pairing-id",
        "other-pairing-id",
        "device-static-key"
    ));
    assert!(!renewal_device_matches(
        &device,
        "module-pairing-id",
        "module-pairing-id",
        "different-device-key"
    ));
}

#[test]
fn abandoning_pairing_clears_only_host_session_capabilities() {
    let channel_id = "bus-test-channel";
    PENDING_SESSIONS
        .lock()
        .unwrap()
        .insert(channel_id.into(), session("2099-01-01T00:00:00Z"));
    PENDING_HANDSHAKES.lock().unwrap().insert(channel_id.into());
    ACTIVE_SESSIONS.lock().unwrap().insert(
        channel_id.into(),
        super::super::rpc::ActiveDeviceSession {
            scope_id: "scope".into(),
            session_id: "module-session".into(),
            pairing_token: "token".into(),
        },
    );

    abandon_unpersisted_pairing(channel_id);

    assert!(!PENDING_SESSIONS.lock().unwrap().contains_key(channel_id));
    assert!(!PENDING_HANDSHAKES.lock().unwrap().contains(channel_id));
    assert!(!ACTIVE_SESSIONS.lock().unwrap().contains_key(channel_id));
}
