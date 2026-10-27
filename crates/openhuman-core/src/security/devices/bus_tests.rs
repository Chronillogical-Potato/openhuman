use super::super::types::PairingSession;
use super::*;
use crate::security::devices::crypto::DeviceKeypair;
use crate::security::devices::lock_unpoisoned;
use std::sync::{Mutex, PoisonError};

fn pending_session(channel_id: &str, expires_at: &str) -> PairingSession {
    PairingSession {
        channel_id: channel_id.to_string(),
        pairing_token: "token".to_string(),
        core_pubkey: "public".to_string(),
        core_private_key_encrypted: "enc2:pending".to_string(),
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
    lock_unpoisoned(&PENDING_KEYPAIRS).insert(
        channel_id.to_string(),
        std::sync::Arc::new(DeviceKeypair::generate()),
    );
    lock_unpoisoned(&PENDING_SESSIONS).insert(
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
    lock_unpoisoned(&PENDING_KEYPAIRS).insert(
        channel_id.to_string(),
        std::sync::Arc::new(DeviceKeypair::generate()),
    );
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
    lock_unpoisoned(&PENDING_KEYPAIRS).insert(
        channel_id.to_string(),
        std::sync::Arc::new(DeviceKeypair::generate()),
    );

    assert!(take_pending_pairing_for_handshake(channel_id, chrono::Utc::now()).is_none());
    assert!(!lock_unpoisoned(&ACTIVE_CIPHERS).contains_key(channel_id));
    lock_unpoisoned(&PENDING_KEYPAIRS).remove(channel_id);
}

#[tokio::test]
async fn a_restarted_host_opens_a_fresh_handshake_from_a_paired_row() {
    let original = DeviceKeypair::generate();
    let private = original.private_bytes();
    let data_dir = tempfile::tempdir().unwrap();
    let config = crate::config::Config::default();
    let secret_store = crate::security::secret_storage::SecretStore::new_with_config(
        data_dir.path(),
        true,
        &config,
    );
    let secret = secret_store
        .encrypt_via_security(&crate::security::devices::crypto::base64url_encode(
            &private,
        ))
        .await
        .unwrap();
    let phone = KeyPair::generate().unwrap();
    let backend = crate::storage::test_support::memory_backend();
    let scoped = backend
        .for_scope(&crate::storage::Scope::new("local").unwrap())
        .unwrap();
    let store = super::super::store_documents::Docs::over(&scoped);
    store
        .insert_device_with_key(
            "restart-channel",
            "phone",
            &phone.public_key().encode(),
            "pairing-hash",
            Some(&secret),
        )
        .unwrap();
    let device = store
        .get_device("restart-channel")
        .unwrap()
        .expect("paired row persisted");
    let restored = restore_live_pairing_key(&device, &secret_store)
        .await
        .unwrap()
        .expect("non-revoked paired identity");
    assert_eq!(restored.pubkey_b64, original.pubkey_b64);

    let client_ephemeral = KeyPair::generate().unwrap();
    let payload = serde_json::to_vec(&json!({
        "device_pubkey": phone.public_key().encode(),
        "client_ephemeral_pubkey": client_ephemeral.public_key().encode(),
    }))
    .unwrap();
    let handshake = phone
        .seal_handshake(PublicKey::decode(&restored.pubkey_b64).unwrap(), &payload)
        .unwrap();
    let (_, plaintext) = restored.open_handshake(&handshake).unwrap();
    let decoded: JsonHandshakePayload = serde_json::from_slice(&plaintext).unwrap();
    assert_eq!(decoded.device_pubkey, phone.public_key().encode());
    assert!(matches_paired_device_key(&device, &decoded.device_pubkey));
    let server_ephemeral = KeyPair::generate().unwrap();
    let client_public = client_ephemeral.public_key();
    let server_public = server_ephemeral.public_key();
    let server_keys = restored
        .session_keys(
            phone.public_key(),
            &server_ephemeral,
            client_public,
            server_public,
        )
        .unwrap();
    let mut server_tunnel = Tunnel::new(Role::Server, server_keys);
    let client_keys = phone
        .session_keys(
            PublicKey::decode(&restored.pubkey_b64).unwrap(),
            &client_ephemeral,
            server_public,
            client_public,
            server_public,
        )
        .unwrap();
    let client_tunnel = Tunnel::new(Role::Client, client_keys);
    let ack = restored
        .seal_ack(
            client_public,
            &serde_json::to_vec(&json!({
                "kind": "handshake_ack",
                "server_ephemeral_pubkey": server_public.encode(),
            }))
            .unwrap(),
        )
        .unwrap();
    assert_eq!(ack[0], 0x04);
    let frame = client_tunnel.seal(b"first post-restart frame").unwrap();
    assert_eq!(
        &*server_tunnel.open(&frame).unwrap(),
        b"first post-restart frame"
    );
    assert!(PENDING_SESSIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&device.channel_id)
        .is_none());
    let after = store
        .get_device(&device.channel_id)
        .unwrap()
        .expect("paired row remains");
    assert_eq!(after.device_pubkey, device.device_pubkey);
    assert_eq!(
        after.core_private_key_encrypted,
        device.core_private_key_encrypted
    );
}

#[tokio::test]
async fn restarted_pairing_rejects_mismatched_or_revoked_device_rows() {
    let original = DeviceKeypair::generate();
    let data_dir = tempfile::tempdir().unwrap();
    let config = crate::config::Config::default();
    let secret_store = crate::security::secret_storage::SecretStore::new_with_config(
        data_dir.path(),
        true,
        &config,
    );
    let secret = "enc2:placeholder".to_string();
    let mut device = super::super::types::PairedDevice {
        channel_id: "restart-channel-revoked".to_string(),
        label: "phone".to_string(),
        device_pubkey: "paired-device-key".to_string(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
        last_seen_at: None,
        peer_online: None,
        revoked: false,
        core_private_key_encrypted: Some(secret),
    };
    assert!(!matches_paired_device_key(&device, "different-device-key"));
    device.revoked = true;
    assert!(restore_live_pairing_key(&device, &secret_store)
        .await
        .unwrap()
        .is_none());
    assert!(!matches_paired_device_key(&device, "paired-device-key"));
}
