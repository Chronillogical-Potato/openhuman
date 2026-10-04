use super::*;

use crate::config::schema::MemoryAgentConfig;

fn config() -> Config {
    Config::default()
}

fn ns(value: &str) -> Namespace {
    value.parse().unwrap()
}

#[test]
fn the_main_agent_is_the_root_and_others_get_their_own_node() {
    let config = config();
    assert_eq!(
        namespace_for(&config, "orchestrator", None),
        Namespace::ROOT
    );
    assert_eq!(
        namespace_for(&config, "researcher", None),
        ns("agent:researcher")
    );
    assert_eq!(
        namespace_for(&config, "writer", Some("acme")),
        ns("team:acme/agent:writer")
    );
    assert_eq!(
        namespace_for(&config, "orchestrator", Some("acme")),
        ns("team:acme"),
        "a root agent in a team writes the team's node"
    );
    assert_eq!(namespace_for(&config, "", None), Namespace::ROOT);
    assert_eq!(MemoryIdentity::root().namespace(&config), Namespace::ROOT);
    assert_eq!(
        namespace_for(&config, "a.b", None).segments()[0].kind(),
        SegmentKind::Agent,
        "an id outside the charset is sanitized, not refused"
    );
}

#[test]
fn config_pins_a_node_and_can_switch_inheritance_off() {
    let mut config = config();
    config.memory.agents.insert(
        "analyst".into(),
        MemoryAgentConfig {
            namespace: Some("project:q4".into()),
            inherit: false,
            context: None,
        },
    );
    config.memory.agents.insert(
        "broken".into(),
        MemoryAgentConfig {
            namespace: Some("not a namespace".into()),
            ..MemoryAgentConfig::default()
        },
    );
    let analyst = MemoryIdentity::agent("analyst");
    assert_eq!(analyst.namespace(&config), ns("project:q4"));
    assert_eq!(analyst.reach(&config), Reach::exact(ns("project:q4")));
    assert_eq!(
        MemoryIdentity::agent("broken").namespace(&config),
        ns("agent:broken"),
        "an invalid pin falls back to the derived node"
    );
    config.memory.root_agents.push("assistant".into());
    assert_eq!(namespace_for(&config, "assistant", None), Namespace::ROOT);
}

#[test]
fn sub_agents_nest_under_their_parent() {
    let config = config();
    let main = MemoryIdentity::agent("orchestrator");
    assert_eq!(main.namespace(&config), Namespace::ROOT);
    let researcher = main.child("researcher");
    assert_eq!(researcher.namespace(&config), ns("agent:researcher"));
    let scout = researcher.child("scout");
    assert_eq!(scout.lineage, ["orchestrator", "researcher"]);
    assert_eq!(scout.namespace(&config), ns("agent:researcher/agent:scout"));
    assert_eq!(
        scout.reach(&config).nodes(),
        vec![
            Namespace::ROOT,
            ns("agent:researcher"),
            ns("agent:researcher/agent:scout")
        ]
    );
    assert_eq!(
        researcher.child("orchestrator").namespace(&config),
        ns("agent:researcher"),
        "a root agent spawned below stays on its parent's node"
    );
    let member = MemoryIdentity::team_member("acme", "writer");
    assert_eq!(member.namespace(&config).shared_ancestor(), ns("team:acme"));
    assert_eq!(
        member.child("editor").namespace(&config),
        ns("team:acme/agent:writer/agent:editor")
    );
}

#[test]
fn deep_spawn_chains_stop_at_the_depth_limit() {
    let config = config();
    let mut identity = MemoryIdentity::root();
    for depth in 0..20 {
        identity = identity.child(&format!("a{depth}"));
    }
    assert_eq!(
        identity.namespace(&config).depth(),
        tinymemory::namespace::MAX_DEPTH
    );
}

#[tokio::test]
async fn turns_scope_the_identity_and_nest_inside_another_agent() {
    assert_eq!(current(), None);
    within_agent("orchestrator", async {
        assert_eq!(current().unwrap().agent_id.as_deref(), Some("orchestrator"));
        within_agent("orchestrator", async {
            assert!(
                current().unwrap().lineage.is_empty(),
                "the same agent keeps its identity"
            );
        })
        .await;
        within_agent("researcher", async {
            let inner = current().unwrap();
            assert_eq!(inner.agent_id.as_deref(), Some("researcher"));
            assert_eq!(inner.lineage, ["orchestrator"]);
        })
        .await;
    })
    .await;
    assert_eq!(current(), None);
}
