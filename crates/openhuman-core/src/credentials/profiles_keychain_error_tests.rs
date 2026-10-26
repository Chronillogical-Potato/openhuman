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

    assert_eq!(store.use_keychain, cfg!(feature = "security-module"));
    assert!(store.security_config.is_some());
    assert_eq!(
        super::profiles::namespaced_keyring_name("user-123", "auth:openai:default"),
        "user-123:auth:openai:default"
    );
}

#[test]
fn unavailable_keyring_keeps_keyring_only_oauth_metadata_in_place() {
    let tmp = TempDir::new().unwrap();
    let config = crate::config::Config::default();
    let store = AuthProfilesStore::new_with_config(tmp.path(), true, &config);
    let profile = AuthProfile::new_oauth(
        "openai-codex",
        "default",
        TokenSet {
            access_token: "not-persisted-here".into(),
            refresh_token: None,
            id_token: None,
            expires_at: None,
            token_type: None,
            scope: None,
        },
    );
    let id = profile.id.clone();
    let persisted = PersistedAuthProfile {
        provider: profile.provider,
        profile_name: profile.profile_name,
        kind: "oauth".into(),
        ..PersistedAuthProfile::default()
    };
    let mut document = PersistedAuthProfiles::default();
    document.profiles.insert(id.clone(), persisted);
    store.write_persisted_locked(&document).unwrap();

    let loaded = store.load().expect("unavailable keyring is handled safely");
    assert!(!loaded.profiles.contains_key(&id));
    assert!(
        store
            .read_persisted_locked()
            .unwrap()
            .profiles
            .contains_key(&id)
    );
}
