use super::*;

fn visible_names(agent_id: &str) -> std::collections::HashSet<String> {
    let tmp = tempfile::TempDir::new().unwrap();
    let config = test_config(&tmp);
    let definition = crate::agent::harness::AgentDefinitionRegistry::builtins_only()
        .get(agent_id)
        .cloned()
        .unwrap_or_else(|| panic!("built-in agent definition not found: {agent_id}"));
    let agent =
        crate::agent::OpenHumanSessionHost::from_config_with_definition(&config, &definition)
            .unwrap_or_else(|e| panic!("{agent_id} session build: {e}"));
    agent
        .visible_tool_specs_arc()
        .iter()
        .map(|spec| spec.name.clone())
        .collect()
}

#[test]
fn resetting_wildcard_visibility_keeps_collapsed_exposure() {
    let tmp = tempfile::TempDir::new().unwrap();
    let config = test_config(&tmp);
    let definition = crate::agent::harness::AgentDefinitionRegistry::builtins_only()
        .get("tools_agent")
        .cloned()
        .expect("tools_agent built-in definition");
    let mut agent =
        crate::agent::OpenHumanSessionHost::from_config_with_definition(&config, &definition)
            .expect("build tools agent");

    agent.set_visible_tool_names(std::collections::HashSet::new());

    let visible = agent.visible_tool_specs_arc();
    assert!(visible
        .iter()
        .any(|spec| spec.name == crate::memory::tools::MEMORY_TOOL_NAME));
    assert!(!visible.iter().any(|spec| spec.name == "memory_store"));
}

#[test]
fn hiding_and_reseeding_wildcard_visibility_keeps_collapsed_exposure() {
    let tmp = tempfile::TempDir::new().unwrap();
    let config = test_config(&tmp);
    let definition = crate::agent::harness::AgentDefinitionRegistry::builtins_only()
        .get("tools_agent")
        .cloned()
        .expect("tools_agent built-in definition");
    let mut agent =
        crate::agent::OpenHumanSessionHost::from_config_with_definition(&config, &definition)
            .expect("build tools agent");

    agent.set_visible_tool_names(std::collections::HashSet::new());
    agent.hide_tools(&[crate::memory::tools::MEMORY_TOOL_NAME]);

    let visible = agent.visible_tool_specs_arc();
    assert!(!visible
        .iter()
        .any(|spec| spec.name == crate::memory::tools::MEMORY_TOOL_NAME));
    assert!(!visible.iter().any(|spec| spec.name == "memory_store"));
}

/// A wildcard belt advertises the collapsed tool, never its `Hidden` members.
///
/// Both halves are asserted: the members gone AND the replacement present. A
/// belt that lost the memory surface entirely would pass a members-only check.
/// Regressed silently once already — `4efbea728` unregistered `memory` and
/// dropped the only production call to `strip_deferred_from_visible`, so every
/// wildcard agent shipped all eleven `memory_*` schemas plus `todo` beside the
/// eight `todo_*` tools it replaces.
#[test]
fn wildcard_belt_advertises_collapsed_tools_not_their_hidden_members() {
    let visible = visible_names("tools_agent");

    for collapsed in [crate::memory::tools::MEMORY_TOOL_NAME, "todo"] {
        assert!(
            visible.contains(collapsed),
            "wildcard belt must advertise `{collapsed}`; got {visible:?}"
        );
    }
    let leaked: Vec<&String> = visible
        .iter()
        .filter(|name| {
            (name.starts_with("memory_") && name.as_str() != "memory_tree")
                || name.starts_with("todo_")
        })
        .collect();
    assert!(
        leaked.is_empty(),
        "Hidden members of `memory`/`todo` must not ship beside them: {leaked:?}"
    );
}

struct FakeTool(&'static str, tinytools::ToolExposure);

#[async_trait::async_trait]
impl tinytools::Tool for FakeTool {
    fn name(&self) -> &str {
        self.0
    }
    fn description(&self) -> &str {
        "fake"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }
    fn exposure(&self) -> tinytools::ToolExposure {
        self.1
    }
    async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<tinytools::ToolResult> {
        Ok(tinytools::ToolResult::success("ok"))
    }
}

fn build_with(
    tools: Vec<Box<dyn tinytools::Tool>>,
    visible: std::collections::HashSet<String>,
) -> crate::agent::OpenHumanSessionHost {
    let model: std::sync::Arc<dyn tinyinference_llm::model::ChatModel<()>> =
        std::sync::Arc::new(tinyagents_harness::testkit::ScriptedModel::new(Vec::new()));
    crate::agent::SessionHostBuilder::new()
        .chat_model(model)
        .tools(tools)
        .visible_tool_names(visible)
        .memory(crate::memory::test_support::noop_memory())
        .tool_dispatcher(Box::new(tinytools_agent::dialect::XmlDialect))
        .build()
        .expect("session build")
}

fn direct_and_deferred() -> Vec<Box<dyn tinytools::Tool>> {
    vec![
        Box::new(FakeTool("plain", tinytools::ToolExposure::Direct)),
        Box::new(FakeTool("rare", tinytools::ToolExposure::Deferred)),
    ]
}

/// A wildcard belt withholds every `Deferred` registration from the wire and
/// keeps it reachable: never advertised, always in the deferred set the
/// harness registers for its `tool_search` bridge, and classified `Allow` by
/// the policy — a found tool the gate refuses as prompt-hidden is the
/// unusable find this replaced.
#[test]
fn wildcard_belt_defers_but_keeps_reachable() {
    let agent = build_with(direct_and_deferred(), std::collections::HashSet::new());

    assert_eq!(
        agent.deferred_tool_names_for_test(),
        &std::collections::HashSet::from(["rare".to_string()])
    );
    assert!(agent.visible_tool_names_for_test().contains("plain"));
    assert!(!agent.visible_tool_names_for_test().contains("rare"));
    assert!(
        !agent
            .visible_tool_specs_arc()
            .iter()
            .any(|spec| spec.name == "rare"),
        "a deferred tool must not be in the prompt's spec list"
    );
    assert!(
        agent.tool_policy_session_for_test().is_allowed("rare"),
        "a deferred tool must be callable once found"
    );
}

/// A named belt that lists `tool_search` opts into discovery: the name itself
/// is the harness's intrinsic bridge and leaves the allowlist, every
/// `Deferred` registration becomes reachable whether or not the belt named
/// it, and the belt's own `Direct` entries stay exactly as written.
#[test]
fn named_belt_opts_into_discovery_by_naming_tool_search() {
    let visible: std::collections::HashSet<String> = [
        "plain",
        crate::tools::implementations::meta::TOOL_SEARCH_NAME,
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let agent = build_with(direct_and_deferred(), visible);

    assert_eq!(
        agent.visible_tool_names_for_test(),
        &std::collections::HashSet::from(["plain".to_string()])
    );
    assert_eq!(
        agent.deferred_tool_names_for_test(),
        &std::collections::HashSet::from(["rare".to_string()])
    );
    assert!(agent.tool_policy_session_for_test().is_allowed("rare"));
}

/// A named belt that does not opt in reaches nothing beyond what it wrote
/// down: no deferred set, and a deferred tool it did not name stays hidden.
#[test]
fn named_belt_without_tool_search_reaches_no_deferred_tool() {
    let visible: std::collections::HashSet<String> =
        std::iter::once("plain".to_string()).collect();
    let agent = build_with(direct_and_deferred(), visible);

    assert!(agent.deferred_tool_names_for_test().is_empty());
    assert!(!agent.tool_policy_session_for_test().is_allowed("rare"));
}

/// A hand-written `[tools] named` belt is left exactly as written: exposure is
/// only for the wildcard belt. `flow_memory_agent` names three read-only
/// `memory_*` tools; swapping them for `memory` would hand it `store`/`forget`.
#[test]
fn named_belt_keeps_its_legacy_members() {
    let visible = visible_names("flow_memory_agent");
    assert!(
        visible.contains("memory_recall"),
        "named belt must keep the members it lists; got {visible:?}"
    );
    assert!(
        !visible.contains(crate::memory::tools::MEMORY_TOOL_NAME),
        "named belt must not gain the collapsed tool; got {visible:?}"
    );
}
