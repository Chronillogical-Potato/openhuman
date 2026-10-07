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

#[test]
fn a_user_root_is_the_account_id_or_a_hashed_local_id() {
    assert_eq!(
        user_root_for("6512AB0F6512ab0f6512ab0f").as_deref(),
        Some("user:6512ab0f6512ab0f6512ab0f")
    );
    let local = user_root_for("local-megamind-macbook").unwrap();
    assert!(local.starts_with("user:local-"), "{local}");
    assert_eq!(local.len(), "user:local-".len() + 16);
    assert!(!local.contains("megamind"), "no device name: {local}");
    assert_eq!(Some(local), user_root_for("local-megamind-macbook"));
    assert_eq!(user_root_for(crate::config::PRE_LOGIN_USER_ID), None);
    assert_eq!(user_root_for(""), None);
}

#[test]
fn the_user_root_is_read_from_where_the_config_lives() {
    let mut config = Config::default();
    config.config_path = "/h/.openhuman/users/6512ab0f6512ab0f6512ab0f/config.toml".into();
    assert_eq!(
        user_root(&config).as_deref(),
        Some("user:6512ab0f6512ab0f6512ab0f")
    );
    config.config_path = "/h/.openhuman/users/local/config.toml".into();
    assert_eq!(user_root(&config), None);
    config.config_path = "/somewhere/else/config.toml".into();
    assert_eq!(user_root(&config), None);
}

#[test]
fn layout_v3_pools_every_agents_chats_at_ws_main() {
    let mut config = Config::default();
    let legacy = MemoryIdentity::agent("researcher").resolve(&config);
    assert_eq!(
        legacy.layout.conversations("researcher").unwrap(),
        ns("agent:researcher")
    );
    assert!(!layout_is_v3(&config));

    config.memory.layout = MemoryLayoutMode::V3;
    assert!(layout_is_v3(&config));
    let researcher = MemoryIdentity::agent("researcher").resolve(&config);
    let coder = MemoryIdentity::agent("coder").resolve(&config);
    assert_eq!(
        researcher.layout.conversations("researcher").unwrap(),
        ns("ws:main")
    );
    assert_eq!(coder.layout.conversations("coder").unwrap(), ns("ws:main"));
    assert_eq!(chat_node(&researcher.layout), ns("ws:main"));
    let history = researcher.layout.conversations_filter(Some("researcher"));
    assert_eq!(history.agent_id.as_deref(), Some("researcher"));

    let team = MemoryIdentity::team_member("acme", "writer").resolve(&config);
    assert_eq!(chat_node(&team.layout), ns("team:acme/ws:main"));
}
