use super::*;

use crate::config::schema::MemoryAgentConfig;

fn ns(value: &str) -> Namespace {
    value.parse().unwrap()
}

#[test]
fn by_default_an_agent_is_its_definition_under_the_default_root() {
    let config = Config::default();
    let researcher = MemoryIdentity::agent("researcher").resolve(&config);
    assert_eq!(researcher.agent_id, "researcher");
    assert_eq!(researcher.root(), &Namespace::ROOT);
    assert!(researcher.recall);

    let nobody = MemoryIdentity::root().resolve(&config);
    assert_eq!(nobody.agent_id, DEFAULT_AGENT_ID);
    assert_eq!(
        MemoryIdentity::agent("  ").resolve(&config).agent_id,
        DEFAULT_AGENT_ID
    );
}

#[test]
fn a_team_member_shares_its_teams_root() {
    let config = Config::default();
    let writer = MemoryIdentity::team_member("acme", "writer").resolve(&config);
    assert_eq!(writer.root(), &ns("team:acme"));
    assert_eq!(writer.agent_id, "writer");
    assert_eq!(
        writer.layout.conversations("writer").unwrap(),
        ns("team:acme/agent:writer")
    );
    let child = MemoryIdentity::team_member("acme", "lead").child("scout");
    assert_eq!(child.resolve(&config).root(), &ns("team:acme"));
}

#[test]
fn a_definition_pin_beats_the_team_and_the_default() {
    let mut config = Config::default();
    config.memory.agents.insert(
        "analyst".into(),
        MemoryAgentConfig {
            agent_id: Some("q4-desk".into()),
            root: Some("project:q4".into()),
            recall: Some(false),
        },
    );
    config.memory.agents.insert(
        "broken".into(),
        MemoryAgentConfig {
            root: Some("not a namespace".into()),
            ..MemoryAgentConfig::default()
        },
    );
    let analyst = MemoryIdentity::team_member("acme", "analyst").resolve(&config);
    assert_eq!(analyst.agent_id, "q4-desk");
    assert_eq!(analyst.root(), &ns("project:q4"));
    assert!(!analyst.recall);
    assert_eq!(
        MemoryIdentity::agent("broken").resolve(&config).root(),
        &Namespace::ROOT,
        "an invalid root falls back to the default"
    );
}

#[test]
fn a_host_binding_beats_everything() {
    let mut config = Config::default();
    config.memory.agent_id = Some("employee-7".into());
    config.memory.root = Some("project:acme".into());
    config.memory.agents.insert(
        "researcher".into(),
        MemoryAgentConfig {
            agent_id: Some("ignored".into()),
            root: Some("project:ignored".into()),
            recall: None,
        },
    );
    for identity in [
        MemoryIdentity::agent("researcher"),
        MemoryIdentity::team_member("acme", "writer"),
        MemoryIdentity::root(),
    ] {
        let resolved = identity.resolve(&config);
        assert_eq!(resolved.agent_id, "employee-7");
        assert_eq!(resolved.root(), &ns("project:acme"));
    }
}

#[tokio::test]
async fn within_agent_scopes_a_child_in_the_same_team() {
    assert_eq!(current(), None);
    let seen = within(MemoryIdentity::team_member("acme", "lead"), async {
        within_agent("lead", async {
            within_agent("scout", async { current().unwrap() }).await
        })
        .await
    })
    .await;
    assert_eq!(seen, MemoryIdentity::team_member("acme", "scout"));
    let alone = within_agent("solo", async { current().unwrap() }).await;
    assert_eq!(alone, MemoryIdentity::agent("solo"));
}

#[test]
fn validate_root_accepts_nodes_and_refuses_junk() {
    assert!(validate_root("team:acme").is_ok());
    assert!(validate_root("project:q4/team:ops").is_ok());
    assert!(validate_root("not a namespace").is_err());
    assert!(validate_root("company:acme").is_err());
}
