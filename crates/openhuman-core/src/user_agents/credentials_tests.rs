use super::*;
use crate::security::credentials::session_support::{
    resolve_backend_credential, BackendCredential,
};
use crate::user_agents::layout::{agent_config, UserAgentLayout};
use crate::user_agents::UserAgentId;

fn agent(tmp: &tempfile::TempDir, user: &str) -> Config {
    let id = UserAgentId::for_user(user).unwrap();
    let layout = UserAgentLayout::new(tmp.path(), &id);
    std::fs::create_dir_all(&layout.workspace_dir).unwrap();
    agent_config(&layout, &id)
}

#[test]
fn each_agent_resolves_only_its_own_credential() {
    let tmp = tempfile::tempdir().unwrap();
    let alice = agent(&tmp, "alice");
    let bob = agent(&tmp, "bob");
    store(&alice, UserCredentialKind::Session, "alice-jwt", None).unwrap();
    store(&bob, UserCredentialKind::ApiKey, "bob-key", None).unwrap();

    assert_eq!(
        resolve_backend_credential(&alice).unwrap(),
        BackendCredential::Session("alice-jwt".into())
    );
    assert_eq!(
        resolve_backend_credential(&bob).unwrap(),
        BackendCredential::ApiKey("bob-key".into())
    );
}

#[test]
fn clearing_one_agent_leaves_the_other() {
    let tmp = tempfile::tempdir().unwrap();
    let alice = agent(&tmp, "alice");
    let bob = agent(&tmp, "bob");
    store(&alice, UserCredentialKind::Session, "alice-jwt", None).unwrap();
    store(&bob, UserCredentialKind::Session, "bob-jwt", None).unwrap();
    assert!(clear(&alice).unwrap());
    assert!(!has(&alice));
    assert!(has(&bob));
    assert!(!clear(&alice).unwrap(), "nothing left to clear");
}

#[test]
fn an_expired_session_is_rejected_locally() {
    let tmp = tempfile::tempdir().unwrap();
    let alice = agent(&tmp, "alice");
    store(
        &alice,
        UserCredentialKind::Session,
        "alice-jwt",
        Some("2000-01-01T00:00:00Z"),
    )
    .unwrap();
    let err = resolve_backend_credential(&alice).unwrap_err();
    assert!(err.contains("SESSION_EXPIRED"), "{err}");
}

#[test]
fn bad_input_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let alice = agent(&tmp, "alice");
    assert!(store(&alice, UserCredentialKind::Session, "  ", None).is_err());
    assert!(store(&alice, UserCredentialKind::Session, "t", Some("tomorrow")).is_err());
    assert!(!has(&alice));
}

#[test]
fn installing_one_kind_replaces_the_other() {
    let tmp = tempfile::tempdir().unwrap();
    let alice = agent(&tmp, "alice-rotates");
    store(&alice, UserCredentialKind::ApiKey, "old-key", None).unwrap();
    store(&alice, UserCredentialKind::Session, "new-jwt", None).unwrap();
    assert_eq!(
        resolve_backend_credential(&alice).unwrap(),
        BackendCredential::Session("new-jwt".into()),
        "the old API key no longer wins"
    );
    store(&alice, UserCredentialKind::ApiKey, "newer-key", None).unwrap();
    assert_eq!(
        resolve_backend_credential(&alice).unwrap(),
        BackendCredential::ApiKey("newer-key".into())
    );
    assert!(clear(&alice).unwrap());
}

#[test]
fn every_profile_of_the_other_kind_is_removed() {
    let tmp = tempfile::tempdir().unwrap();
    let alice = agent(&tmp, "alice-profiles");
    // A non-default, active API-key profile.
    AuthService::from_config(&alice)
        .store_provider_token(
            api_key::API_KEY_PROVIDER,
            "other",
            "side-key",
            HashMap::new(),
            true,
        )
        .unwrap();
    store(&alice, UserCredentialKind::Session, "jwt", None).unwrap();
    assert_eq!(
        resolve_backend_credential(&alice).unwrap(),
        BackendCredential::Session("jwt".into())
    );
    assert!(clear(&alice).unwrap());
    assert!(!has(&alice));
}
