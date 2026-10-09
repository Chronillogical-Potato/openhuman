use super::*;

#[cfg(feature = "mcp")]
#[test]
fn declaring_mcp_servers_opts_a_named_belt_into_tool_search() {
    use openhuman_core::agent::harness::definition::ToolScope;

    let mut named = ToolScope::Named(vec!["read_file".into()]);
    opt_named_belt_into_discovery(&mut named);
    opt_named_belt_into_discovery(&mut named);
    assert!(matches!(
        named,
        ToolScope::Named(ref names) if names == &["read_file".to_string(), "tool_search".to_string()]
    ));

    let mut wildcard = ToolScope::Wildcard;
    opt_named_belt_into_discovery(&mut wildcard);
    assert!(matches!(wildcard, ToolScope::Wildcard));
}

#[test]
fn agent_ids_must_be_plain_names() {
    assert!(validate_agent_id("ok-agent_1").is_ok());
    assert!(validate_agent_id("../escape").is_err());
}
