//! Current host crypto boundary characterization.
//!
//! These tests pin the host adapter's persisted-device round trip and committed
//! encrypted-file fixture bytes without making backend or network calls.

use openhuman_core::security::devices::crypto::{
    base64url_decode, base64url_encode, DeviceKeypair,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

const CORPUS: &str = include_str!("fixtures/security-crypto-corpus.json");
const SECRETS_ENC: &[u8] =
    include_bytes!("../crates/openhuman-core/src/security/secret_storage/fixtures/secrets_enc.bin");
const ENC2_VALUES: &[u8] =
    include_bytes!("../crates/openhuman-core/src/security/secret_storage/fixtures/enc2_values.txt");

fn corpus() -> Value {
    serde_json::from_str(CORPUS).expect("valid crypto characterization fixture")
}

#[test]
fn persisted_device_identity_round_trips_through_the_host_adapter() {
    let original = DeviceKeypair::generate();
    let private = original.private_bytes();
    let public = original.pubkey_b64.clone();

    let decoded = base64url_decode(&public).expect("public key uses base64url");
    assert_eq!(decoded.len(), 32);
    assert!(
        !public.contains('='),
        "public key is encoded without padding"
    );
    assert_eq!(base64url_encode(&decoded), public);

    let restored = DeviceKeypair::from_private_bytes(Zeroizing::new(private))
        .expect("persisted private bytes restore the same identity");
    assert_eq!(restored.pubkey_b64, public);
    assert_eq!(restored.private_bytes(), private);
}

#[test]
fn zero_device_private_key_is_rejected() {
    assert!(DeviceKeypair::from_private_bytes(Zeroizing::new([0; 32])).is_err());
}

#[test]
fn committed_encrypted_file_keyring_fixture_bytes_are_unchanged() {
    let keyring = &corpus()["keyring"];
    let digest = |bytes: &[u8]| hex::encode(Sha256::digest(bytes));
    assert_eq!(
        digest(SECRETS_ENC),
        keyring["secrets_enc_sha256"]
            .as_str()
            .expect("fixture digest")
    );
    assert_eq!(
        digest(ENC2_VALUES),
        keyring["enc2_values_sha256"]
            .as_str()
            .expect("fixture digest")
    );
    for line in std::str::from_utf8(ENC2_VALUES).unwrap().lines() {
        assert!(line.starts_with("enc2:"));
    }
}
