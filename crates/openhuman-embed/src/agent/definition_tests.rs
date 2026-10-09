use super::*;

#[test]
fn default_is_the_orchestrator_under_the_agents_id() {
    let def = AgentDefinitionSpec::new()
        .into_core("alpha")
        .expect("definition");
    assert_eq!(def.id, "alpha");
    assert_eq!(def.display_name(), "alpha");
    assert!(matches!(def.system_prompt, PromptSource::Dynamic(_)));
    assert!(matches!(def.tools, ToolScope::Wildcard));
    assert_eq!(def.sandbox_mode, SandboxMode::None);
}

#[test]
fn setters_override_one_aspect_each() {
    let def = AgentDefinitionSpec::new()
        .system_prompt("Be terse.")
        .tools(ToolScopeSpec::Named(vec!["read_file".into()]))
        .disallow_tools(["shell"])
        .sandbox(SandboxModeSpec::Sandboxed)
        .max_iterations(3)
        .temperature(0.1)
        .display_name("Alpha")
        .into_core("alpha")
        .expect("definition");
    assert!(matches!(def.system_prompt, PromptSource::Inline(ref p) if p == "Be terse."));
    assert!(
        matches!(def.tools, ToolScope::Named(ref names) if names == &["read_file".to_string()])
    );
    assert!(def.disallowed_tools.iter().any(|t| t == "shell"));
    assert_eq!(def.sandbox_mode, SandboxMode::Sandboxed);
    assert_eq!(def.max_iterations, 3);
    assert_eq!(def.temperature, 0.1);
    assert_eq!(def.display_name(), "Alpha");
}

#[test]
fn bare_prompt_is_verbatim_with_nothing_composed_around_it() {
    let def = AgentDefinitionSpec::new()
        .bare_prompt("Review.")
        .into_core("alpha")
        .expect("definition");
    assert!(matches!(def.system_prompt, PromptSource::Verbatim(ref p) if p == "Review."));
    assert!(def.omit_identity && def.omit_safety_preamble && def.omit_memory_context);
}

#[test]
fn system_prompt_after_bare_prompt_is_wrapped_again() {
    let def = AgentDefinitionSpec::new()
        .bare_prompt("Review.")
        .system_prompt("Be terse.")
        .into_core("alpha")
        .expect("definition");
    assert!(matches!(def.system_prompt, PromptSource::Inline(ref p) if p == "Be terse."));
}

#[test]
fn host_only_is_an_empty_read_only_belt_that_cannot_delegate() {
    let def = AgentDefinitionSpec::new()
        .bare_prompt("Review.")
        .tools(ToolScopeSpec::HostOnly)
        .sandbox(SandboxModeSpec::None)
        .into_core("alpha")
        .expect("definition");
    assert!(matches!(def.tools, ToolScope::Named(ref names) if names.is_empty()));
    assert_eq!(def.sandbox_mode, SandboxMode::ReadOnly);
    assert!(def.subagents.is_empty());
}

#[test]
fn host_only_without_a_prompt_is_refused() {
    let err = AgentDefinitionSpec::new()
        .tools(ToolScopeSpec::HostOnly)
        .into_core("alpha")
        .expect_err("the orchestrator prompt does not describe a host-only agent");
    assert!(matches!(err, AgentError::Invalid(_)));
}
