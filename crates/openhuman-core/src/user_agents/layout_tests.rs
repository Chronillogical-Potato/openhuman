use super::*;

fn id() -> UserAgentId {
    UserAgentId::for_user("layout-user").unwrap()
}

#[test]
fn every_path_sits_under_the_agent_directory() {
    let layout = UserAgentLayout::new(Path::new("/srv/oh"), &id());
    assert_eq!(layout.dir, Path::new("/srv/oh/agents").join(id().as_str()));
    for path in [
        &layout.meta_path,
        &layout.config_path,
        &layout.workspace_dir,
        &layout.sandbox_dir,
    ] {
        assert!(path.starts_with(&layout.dir), "{}", path.display());
    }
}

#[test]
fn the_config_is_rooted_in_the_agent_and_bound_to_its_memory() {
    let layout = UserAgentLayout::new(Path::new("/srv/oh"), &id());
    let config = agent_config(&layout, &id());
    assert_eq!(config.config_path, layout.config_path);
    assert_eq!(config.workspace_dir, layout.workspace_dir);
    assert_eq!(config.action_dir, layout.sandbox_dir);
    assert_eq!(config.memory.agent_id.as_deref(), Some(id().as_str()));
    assert_eq!(config.memory.root, Some(memory_root(&id())));
}

#[test]
fn the_memory_root_is_a_valid_layout_root() {
    crate::memory::scope::validate_root(&memory_root(&id())).unwrap();
}

#[test]
fn the_memory_binding_wins_over_pins_and_teams() {
    use crate::memory::scope::MemoryIdentity;
    let layout = UserAgentLayout::new(Path::new("/srv/oh"), &id());
    let mut config = agent_config(&layout, &id());
    config.memory.agents.insert(
        "planner".into(),
        toml::from_str("agent_id = \"other\"\nroot = \"team:shared\"").unwrap(),
    );
    let resolved = MemoryIdentity::team_member("shared", "planner").resolve(&config);
    assert_eq!(resolved.agent_id, id().as_str());
    assert_eq!(resolved.root().to_string(), memory_root(&id()));
}

#[test]
fn the_policy_is_on_and_closed() {
    let layout = UserAgentLayout::new(Path::new("/srv/oh"), &id());
    let autonomy = agent_config(&layout, &id()).autonomy;
    assert!(autonomy.enabled);
    assert_eq!(autonomy.level, AutonomyLevel::Supervised);
    assert!(autonomy.workspace_only);
    assert!(!autonomy.auto_approve_all);
    assert!(!autonomy.allow_tool_install);
    assert!(autonomy.trusted_roots.is_empty());
}
