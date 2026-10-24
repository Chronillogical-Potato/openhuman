use super::*;
use crate::security::devices::lock_unpoisoned;
use crate::security::devices::crypto::DeviceKeypair;
use std::sync::{Mutex, PoisonError};
use super::super::types::PairingSession;

fn pending_session(channel_id: &str, expires_at: &str) -> PairingSession {
    PairingSession {
        channel_id: channel_id.to_string(),
        pairing_token: "token".to_string(),
        core_pubkey: "public".to_string(),
        rpc_url: None,
        expires_at: expires_at.to_string(),
        agent: None,
    }
}

#[test]
fn poisoned_device_state_lock_is_recovered_for_the_next_frame() {
    let state = Mutex::new(vec!["paired"]);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = state.lock().unwrap_or_else(PoisonError::into_inner);
        panic!("simulated frame panic");
    }));
    assert!(panic.is_err());

    assert_eq!(&*lock_unpoisoned(&state), &["paired"]);
}

#[test]
fn expired_pending_session_is_discarded_before_handshake() {
    let channel_id = "expired-session-test";
    lock_unpoisoned(&PENDING_KEYPAIRS)
        .insert(channel_id.to_string(), std::sync::Arc::new(DeviceKeypair::generate()));
    lock_unpoisoned(&PENDING_SESSIONS)
        .insert(
            channel_id.to_string(),
            pending_session(channel_id, "2020-01-01T00:00:00Z"),
        );

    assert!(take_pending_pairing_for_handshake(
        channel_id,
        chrono::DateTime::parse_from_rfc3339("2021-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
    )
    .is_none());
    assert!(!lock_unpoisoned(&PENDING_KEYPAIRS).contains_key(channel_id));
    assert!(!lock_unpoisoned(&PENDING_SESSIONS).contains_key(channel_id));
}

#[test]
fn handshake_claim_consumes_its_pending_session_and_keypair_once() {
    let channel_id = "single-use-session-test";
    lock_unpoisoned(&PENDING_KEYPAIRS)
        .insert(channel_id.to_string(), std::sync::Arc::new(DeviceKeypair::generate()));
    lock_unpoisoned(&PENDING_SESSIONS).insert(
        channel_id.to_string(),
        pending_session(channel_id, "2030-01-01T00:00:00Z"),
    );

    assert!(take_pending_pairing_for_handshake(channel_id, chrono::Utc::now()).is_some());
    assert!(take_pending_pairing_for_handshake(channel_id, chrono::Utc::now()).is_none());
    assert!(!lock_unpoisoned(&PENDING_KEYPAIRS).contains_key(channel_id));
    assert!(!lock_unpoisoned(&PENDING_SESSIONS).contains_key(channel_id));
}

#[test]
fn missing_pending_session_stops_handshake_before_cipher_installation() {
    let channel_id = "missing-session-test";
    lock_unpoisoned(&PENDING_KEYPAIRS)
        .insert(channel_id.to_string(), std::sync::Arc::new(DeviceKeypair::generate()));

    assert!(take_pending_pairing_for_handshake(channel_id, chrono::Utc::now()).is_none());
    assert!(!lock_unpoisoned(&ACTIVE_CIPHERS).contains_key(channel_id));
    lock_unpoisoned(&PENDING_KEYPAIRS).remove(channel_id);
}
