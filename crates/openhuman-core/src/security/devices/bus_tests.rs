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
fn abandoning_pairing_clears_only_host_session_capabilities() {
    let channel_id = "bus-test-channel";
    PENDING_SESSIONS
        .lock()
        .unwrap()
        .insert(channel_id.into(), session("2099-01-01T00:00:00Z"));
    PENDING_HANDSHAKES
        .lock()
        .unwrap()
        .insert(channel_id.into(), ());
    ACTIVE_SESSIONS.lock().unwrap().insert(
        channel_id.into(),
        super::super::rpc::ActiveDeviceSession {
            scope_id: "scope".into(),
            session_id: "module-session".into(),
        },
    );

    abandon_unpersisted_pairing(channel_id);

    assert!(!PENDING_SESSIONS.lock().unwrap().contains_key(channel_id));
    assert!(!PENDING_HANDSHAKES.lock().unwrap().contains(channel_id));
    assert!(!ACTIVE_SESSIONS.lock().unwrap().contains_key(channel_id));
}
