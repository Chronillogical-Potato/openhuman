//! Profile keyring configuration stays on the TinySecurity path.

use super::*;
use tempfile::TempDir;

#[test]
fn no_config_profile_store_uses_its_encrypted_document_without_keyring_bypass() {
    let tmp = TempDir::new().unwrap();
    let store = AuthProfilesStore::new(tmp.path(), true);

    assert!(!store.use_keychain);
    assert!(store.security_config.is_none());
}

#[test]
fn config_backed_profile_store_uses_tinysecurity_and_existing_namespaced_key() {
    let tmp = TempDir::new().unwrap();
    let config = crate::config::Config::default();
    let store = AuthProfilesStore::new_with_config(tmp.path(), true, &config);

    assert!(store.use_keychain);
    assert!(store.security_config.is_some());
    assert_eq!(
        super::profiles::namespaced_keyring_name("user-123", "auth:openai:default"),
        "user-123:auth:openai:default"
    );
}
