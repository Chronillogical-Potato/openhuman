use super::*;

#[test]
fn gemini_key_round_trips_with_encryption_enabled() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.config_path = dir.path().join("config.toml");
    config.secrets.encrypt = true;
    config.search.gemini.api_key = Some("gemini-sentinel".into());

    encrypt_config_secrets(&mut config).unwrap();
    let ciphertext = config.search.gemini.api_key.as_deref().unwrap();
    assert!(ciphertext.starts_with("enc2:"));
    assert!(!ciphertext.contains("gemini-sentinel"));
    assert!(!decrypt_config_secrets(&mut config, dir.path()).unwrap());
    assert_eq!(
        config.search.gemini.api_key.as_deref(),
        Some("gemini-sentinel")
    );
}

#[test]
fn storage_url_round_trips_and_fails_closed_without_the_key() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.config_path = dir.path().join("config.toml");
    config.secrets.encrypt = true;
    config.storage.url = Some("mongodb://app:hunter2@db.internal/openhuman".into());

    encrypt_config_secrets(&mut config).unwrap();
    let sealed = config.storage.url.clone().unwrap();
    assert!(sealed.starts_with("enc2:") && !sealed.contains("hunter2"));
    decrypt_config_secrets(&mut config, dir.path()).unwrap();
    assert_eq!(
        config.storage.url.as_deref(),
        Some("mongodb://app:hunter2@db.internal/openhuman")
    );

    // A key that cannot open the URL must not clear it into "no URL".
    let other = tempfile::tempdir().unwrap();
    config.storage.url = Some(sealed.clone());
    decrypt_config_secrets(&mut config, other.path()).unwrap();
    assert_eq!(config.storage.url.as_deref(), Some(sealed.as_str()));
}
