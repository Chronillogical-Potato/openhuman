use super::*;
use crate::core::runtime::{CoreContext, DomainSet, SaasConfig};

fn host(tmp: &tempfile::TempDir) -> AgentHost {
    AgentHost::new(
        SaasConfig::new(tmp.path()),
        CoreContext::for_test(DomainSet::full(), None),
    )
}

#[test]
fn provision_derives_the_agent_and_never_echoes_the_user_id() {
    let tmp = tempfile::tempdir().unwrap();
    let host = host(&tmp);
    let out = provision_on(&host, "alice@example.com").unwrap();
    let json = out.into_cli_compatible_json().unwrap().to_string();
    assert!(!json.contains("alice"), "{json}");
    assert!(json.contains("\"created\":true"), "{json}");
    let again = provision_on(&host, "alice@example.com").unwrap();
    assert!(again
        .into_cli_compatible_json()
        .unwrap()
        .to_string()
        .contains("\"created\":false"));
}

#[test]
fn status_and_deprovision_take_agent_ids_only() {
    let tmp = tempfile::tempdir().unwrap();
    let host = host(&tmp);
    assert!(status_on(&host, "alice@example.com").is_err());
    assert!(deprovision_on(&host, "../operator").is_err());

    let id = UserAgentId::for_user("alice").unwrap();
    assert!(status_on(&host, id.as_str())
        .unwrap_err()
        .contains("not provisioned"));
    provision_on(&host, "alice").unwrap();
    status_on(&host, id.as_str()).unwrap();
    deprovision_on(&host, id.as_str()).unwrap();
    assert!(status_on(&host, id.as_str()).is_err());
}

#[test]
fn credentials_are_set_and_cleared_per_agent_and_never_echoed() {
    let tmp = tempfile::tempdir().unwrap();
    let host = host(&tmp);
    // The keyring holding credential secrets is shared by every test here.
    let alice_user = format!("alice-{}", uuid::Uuid::new_v4());
    let bob_user = format!("bob-{}", uuid::Uuid::new_v4());
    let alice = UserAgentId::for_user(&alice_user).unwrap();
    let bob = UserAgentId::for_user(&bob_user).unwrap();
    assert!(set_credential_on(
        &host,
        alice.as_str(),
        UserCredentialKind::Session,
        "t",
        None
    )
    .unwrap_err()
    .contains("not provisioned"));
    provision_on(&host, &alice_user).unwrap();
    provision_on(&host, &bob_user).unwrap();

    let out = set_credential_on(
        &host,
        alice.as_str(),
        UserCredentialKind::Session,
        "alice-secret-jwt",
        None,
    )
    .unwrap();
    let json = out.into_cli_compatible_json().unwrap().to_string();
    assert!(!json.contains("alice-secret-jwt"), "{json}");
    assert!(host.summary(&alice).unwrap().unwrap().has_credential);
    assert!(!host.summary(&bob).unwrap().unwrap().has_credential);

    clear_credential_on(&host, alice.as_str()).unwrap();
    assert!(!host.summary(&alice).unwrap().unwrap().has_credential);
}
