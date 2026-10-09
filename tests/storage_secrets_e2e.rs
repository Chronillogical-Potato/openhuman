//! Keyring secrets and the credential stores on a configured storage
//! backend, end to end through their public functions.
//!
//! Its own test binary because it installs a backend into the process-wide
//! storage slot and sets the keyring master-key environment, which would
//! reroute every other suite's secrets in a shared process. One test, so
//! nothing in this binary races either.

use std::sync::Arc;

use openhuman_core::security::credentials::http_creds::{HttpCredential, HttpCredentialsStore};
use openhuman_core::security::credentials::profiles::{AuthProfile, AuthProfilesStore};
use openhuman_core::security::keyring;

#[test]
fn a_configured_backend_holds_keyring_and_credential_secrets() {
    let workspace = tempfile::tempdir().unwrap();
    // Keep any process-backend fallback inside the temp workspace, and give
    // the storage secrets a master key without touching an OS keychain.
    std::env::set_var("OPENHUMAN_WORKSPACE", workspace.path());
    std::env::set_var("OPENHUMAN_KEYRING_BACKEND", "file");
    std::env::set_var("OPENHUMAN_KEYRING_MASTER_KEY", "11".repeat(32));

    // Credentials written before any backend existed (the classic files).
    let state = workspace.path().join("state");
    AuthProfilesStore::new(&state, false)
        .upsert_profile(
            AuthProfile::new_token("legacy", "default", "sk-legacy".to_string()),
            true,
        )
        .unwrap();
    HttpCredentialsStore::new(&state, false)
        .upsert(&HttpCredential::bearer("legacy-http", "ghp-legacy"))
        .unwrap();
    keyring::set("legacy-user", "old_token", "old-value").unwrap();
    let profiles_file = std::fs::read(state.join("auth-profiles.json")).unwrap();
    let http_file = std::fs::read(state.join("http-credentials.json")).unwrap();

    let before: std::collections::HashMap<&str, Option<Vec<u8>>> =
        ["secrets.enc", "dev-keychain.json"]
            .into_iter()
            .map(|f| (f, std::fs::read(workspace.path().join(f)).ok()))
            .collect();

    openhuman_core::storage::install(Arc::new(openhuman_core::storage::MemoryStorage::new()));

    keyring::set("user-1", "api_token", "tok-123").unwrap();
    assert_eq!(
        keyring::get("user-1", "api_token").unwrap().as_deref(),
        Some("tok-123")
    );
    assert!(keyring::is_available());
    keyring::delete("user-1", "api_token").unwrap();
    assert!(keyring::get("user-1", "api_token").unwrap().is_none());

    // A secret the process keyring held is adopted on first read, and a
    // delete removes it from both places so it cannot resurface.
    assert_eq!(
        keyring::get("legacy-user", "old_token").unwrap().as_deref(),
        Some("old-value")
    );
    keyring::delete("legacy-user", "old_token").unwrap();
    assert!(keyring::get("legacy-user", "old_token").unwrap().is_none());

    let profiles = AuthProfilesStore::new(&state, false);
    profiles
        .upsert_profile(
            AuthProfile::new_token("openai", "default", "sk-test".to_string()),
            true,
        )
        .unwrap();
    // The pre-existing profile was adopted, not replaced.
    let loaded = profiles.load().unwrap();
    assert_eq!(loaded.profiles.len(), 2);

    let http = HttpCredentialsStore::new(&state, false);
    http.upsert(&HttpCredential::bearer("github", "ghp-test"))
        .unwrap();
    assert!(http.get("github").unwrap().is_some());
    assert!(http.get("legacy-http").unwrap().is_some());

    // The legacy files are left untouched by the backend path.
    assert_eq!(
        std::fs::read(state.join("auth-profiles.json")).unwrap(),
        profiles_file
    );
    assert_eq!(
        std::fs::read(state.join("http-credentials.json")).unwrap(),
        http_file
    );
    // `secrets.enc` was not touched, and nothing of the storage-side secret
    // reached the process keychain file.
    assert_eq!(
        std::fs::read(workspace.path().join("secrets.enc")).ok(),
        before.get("secrets.enc").cloned().flatten(),
        "secrets.enc unchanged"
    );
    let dev = std::fs::read_to_string(workspace.path().join("dev-keychain.json")).unwrap();
    assert!(!dev.contains("user-1") && !dev.contains("tok-123"), "{dev}");

    // Without a backend the files are back in use.
    assert!(openhuman_core::storage::clear());
    assert!(http.get("github").unwrap().is_none());
    assert!(http.get("legacy-http").unwrap().is_some());
    assert_eq!(profiles.load().unwrap().profiles.len(), 1);
}
