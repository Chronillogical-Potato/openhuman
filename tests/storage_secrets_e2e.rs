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
    openhuman_core::storage::install(Arc::new(openhuman_core::storage::MemoryStorage::new()));

    keyring::set("user-1", "api_token", "tok-123").unwrap();
    assert_eq!(
        keyring::get("user-1", "api_token").unwrap().as_deref(),
        Some("tok-123")
    );
    assert!(keyring::is_available());
    keyring::delete("user-1", "api_token").unwrap();
    assert!(keyring::get("user-1", "api_token").unwrap().is_none());

    let state = workspace.path().join("state");
    let profiles = AuthProfilesStore::new(&state, false);
    profiles
        .upsert_profile(
            AuthProfile::new_token("openai", "default", "sk-test".to_string()),
            true,
        )
        .unwrap();
    let loaded = profiles.load().unwrap();
    assert_eq!(loaded.profiles.len(), 1);

    let http = HttpCredentialsStore::new(&state, false);
    http.upsert(&HttpCredential::bearer("github", "ghp-test"))
        .unwrap();
    assert!(http.get("github").unwrap().is_some());

    for file in [
        "state/auth-profiles.json",
        "state/http-credentials.json",
        "secrets.enc",
        "dev-keychain.json",
    ] {
        assert!(
            !workspace.path().join(file).exists(),
            "{file} was not written"
        );
    }

    // Without a backend the files are back in use.
    assert!(openhuman_core::storage::clear());
    assert!(http.get("github").unwrap().is_none());
    assert!(profiles.load().unwrap().profiles.is_empty());
}
