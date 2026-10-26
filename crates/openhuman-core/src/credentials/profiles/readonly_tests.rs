use super::*;
use std::collections::BTreeMap;
use tempfile::TempDir;

#[test]
fn read_only_profile_loads_secrets_stored_in_tinysecurity_keyring() {
    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path();
    let mut config = Config::default();
    config.config_path = state_dir.join("config.toml");
    config.workspace_dir = state_dir.to_path_buf();

    let id = "openai-codex:default";
    let mut persisted = PersistedAuthProfiles::default();
    persisted.profiles.insert(
        id.into(),
        PersistedAuthProfile {
            provider: "openai-codex".into(),
            profile_name: "default".into(),
            kind: "oauth".into(),
            ..PersistedAuthProfile::default()
        },
    );
    persisted.active_profiles = BTreeMap::from([("openai-codex".into(), id.into())]);
    let document = serde_json::to_vec(&persisted).unwrap();
    let directory = profiles_file(state_dir).parent().unwrap();
    std::fs::create_dir_all(directory).unwrap();
    let store = EncryptedFileSecrets::with_key_file(directory).unwrap();
    crate::storage::block_on(async move {
        store
            .set(PROFILE_DOCUMENT_SECRET_NAME, &document)
            .await
            .unwrap();
    });

    let loaded = load_profile_with_keyring(
        &config,
        "openai-codex",
        None,
        |_, _| {
            Ok(Some(KeychainSecrets {
                access_token: Some("keyring-access-token".into()),
                ..KeychainSecrets::default()
            }))
        },
    )
    .expect("keyring-backed profile should be readable")
    .expect("profile should be selected");

    assert_eq!(loaded.id, id);
    assert_eq!(
        loaded.token_set.unwrap().access_token,
        "keyring-access-token"
    );
}
