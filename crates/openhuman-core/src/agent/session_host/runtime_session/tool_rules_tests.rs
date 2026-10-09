use super::*;

fn config_with_rules(tmp: &tempfile::TempDir) -> crate::config::Config {
    let config = crate::config::Config {
        workspace_dir: tmp.path().join("workspace"),
        action_dir: tmp.path().join("workspace"),
        config_path: tmp.path().join("config.toml"),
        tool_rules: tinytools::ToolRules::from_allow_deny(Vec::<String>::new(), ["shell"]),
        ..crate::config::Config::default()
    };
    std::fs::create_dir_all(&config.workspace_dir).unwrap();
    config
}

#[test]
fn a_session_composes_the_operator_and_definition_layers() {
    let tmp = tempfile::TempDir::new().unwrap();
    let config = config_with_rules(&tmp);
    let mut definition = crate::agent::registry::agents::load_builtins()
        .unwrap()
        .into_iter()
        .find(|def| def.id == "orchestrator")
        .expect("orchestrator");
    definition.disallowed_tools = vec!["web_*".to_string()];
    let host = OpenHumanSessionHost::from_config_with_definition(&config, &definition)
        .expect("session builds");

    let rules = host.session_tool_rules();
    let names: Vec<_> = rules
        .layers
        .iter()
        .map(|layer| layer.name.clone().unwrap_or_default())
        .collect();
    assert_eq!(names, ["config", "agent:orchestrator"]);
    let context = tinytools::RuleContext::new();
    let visible = |name: &str| {
        rules.visible(
            &tinytools::ToolSubject::named(name),
            &context,
            tinytools::Surface::Catalog,
        )
    };
    assert!(!visible("shell"));
    assert!(!visible("web_fetch"));
    assert!(visible("file_read"));
}
