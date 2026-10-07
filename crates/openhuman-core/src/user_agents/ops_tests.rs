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
