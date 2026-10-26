use super::*;

#[test]
fn persisted_profile_document_is_encrypted_at_rest() {
    let tmp = tempfile::TempDir::new().unwrap();
    let store = AuthProfilesStore::new(tmp.path(), true);
    store
        .upsert_profile(
            AuthProfile::new_token("openai", "default", "profile-secret-token".into()),
            true,
        )
        .unwrap();

    let bytes = std::fs::read(store.path()).expect("encrypted profile document");
    assert!(!String::from_utf8_lossy(&bytes).contains("profile-secret-token"));
    assert!(serde_json::from_slice::<PersistedAuthProfiles>(&bytes).is_err());
    assert_eq!(
        store.load().unwrap().profiles["openai:default"].token.as_deref(),
        Some("profile-secret-token")
    );
}
