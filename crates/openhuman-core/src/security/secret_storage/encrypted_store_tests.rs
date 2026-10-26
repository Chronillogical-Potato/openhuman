use super::*;
use tempfile::TempDir;

fn hex_encode(data: &[u8]) -> String {
    ::hex::encode(data)
}

fn hex_decode(hex: &str) -> anyhow::Result<Vec<u8>> {
    ::hex::decode(hex).map_err(anyhow::Error::from)
}

#[path = "encrypted_store_crypto_migration_tests.rs"]
mod crypto_migration_tests;

#[tokio::test]
async fn encrypted_secret_operations_fail_closed_without_the_security_module() {
    let tmp = TempDir::new().unwrap();
    let mut config = crate::config::Config::default();
    config.modules.enabled = false;
    let store = SecretStore::new_with_config(tmp.path(), true, &config);

    let error = store
        .encrypt_via_security("do-not-fall-back-to-host-crypto")
        .await
        .expect_err("enabled secret encryption must require TinySecurity");

    assert!(error.to_string().contains("unavailable"));
}

#[tokio::test]
async fn explicit_key_decryption_does_not_fall_back_when_tinysecurity_is_unavailable() {
    let tmp = TempDir::new().unwrap();
    let mut config = crate::config::Config::default();
    config.modules.enabled = false;
    let store = SecretStore::new_with_config(tmp.path(), true, &config);

    let error = store
        .decrypt_with_existing_key_via_security(&format!("enc2:{}", "00".repeat(32)), &"ab".repeat(32))
        .await
        .expect_err("explicit-key decryption must require TinySecurity");

    assert!(error.to_string().contains("unavailable"));
}
