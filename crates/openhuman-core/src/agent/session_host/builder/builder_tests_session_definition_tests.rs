//! `agent_definition`: a direct builder caller can be its own catalogue.
//!
//! Every session turn is a hosted root invocation that resolves its agent id
//! against the host catalogue before it composes a message, and refuses the
//! turn when the id is not there. `from_config_with_definition` already
//! stamps a caller's definition on the session for that reason; a direct
//! builder caller had no equivalent, and fell back to the process registry.
//! These tests pin the route that closes that gap.

use crate::agent::harness::definition::{AgentDefinition, AgentDefinitionRegistry, ToolScope};
use crate::agent::tinyagents::host::OpenHumanDefinitionRegistry;
use std::sync::Arc;
use tinyagents_definition::DefinitionRegistry;

/// An id no built-in definition uses, so a hit can only have come from the
/// caller's own definition.
const SEAT: &str = "library-host--seat";

fn definition(id: &str, tools: Vec<&str>) -> Arc<AgentDefinition> {
    let mut def = AgentDefinitionRegistry::builtins_only()
        .get("orchestrator")
        .cloned()
        .expect("built-in orchestrator");
    def.id = id.to_string();
    def.display_name = Some("Library host seat".to_string());
    def.when_to_use = "a seat a library host built and owns".to_string();
    def.tools = ToolScope::Named(tools.into_iter().map(str::to_string).collect());
    def.disallowed_tools.clear();
    Arc::new(def)
}

fn seat(
    definition: Option<Arc<AgentDefinition>>,
) -> crate::agent::session_host::types::OpenHumanSessionHost {
    let model: Arc<dyn tinyinference_llm::model::ChatModel<()>> =
        Arc::new(tinyagents_harness::testkit::ScriptedModel::new(Vec::new()));
    let mut builder = crate::agent::SessionHostBuilder::new()
        .chat_model(model)
        .tools(Vec::new())
        .memory(crate::memory::test_support::noop_memory())
        .tool_dispatcher(Box::new(tinytools_agent::dialect::XmlDialect))
        .agent_definition_name(SEAT);
    if let Some(definition) = definition {
        builder = builder.agent_definition(definition);
    }
    builder
        .build()
        .expect("a library host seat is a legal build")
}

/// The catalogue a turn is resolved against, assembled the way
/// `OpenHumanHostBundleFactory::build` assembles it.
fn catalogue(
    agent: &crate::agent::session_host::types::OpenHumanSessionHost,
) -> OpenHumanDefinitionRegistry {
    let base = agent
        .hosted_base
        .as_ref()
        .expect("a caller that brought its own definition has a hosted authority");
    let mut catalogue = OpenHumanDefinitionRegistry::new(Arc::clone(&base.definitions))
        .with_config(Arc::clone(&base.config));
    if let Some(definition) = base.session_definition.as_ref() {
        catalogue = catalogue.with_session_definition(Arc::clone(definition));
    }
    catalogue
}

#[tokio::test]
async fn a_definition_the_builder_was_given_resolves_for_the_turn() {
    let agent = seat(Some(definition(SEAT, vec!["desk_say", "publish_artifact"])));

    let resolved = catalogue(&agent)
        .resolve(SEAT)
        .await
        .expect("resolution never errors")
        .expect("the caller's own definition answers the lookup");

    assert_eq!(resolved.id, SEAT);
    assert_eq!(resolved.name, "Library host seat");
    assert!(
        resolved.is_valid(),
        "a hosted turn refuses an invalid definition: {:?}",
        resolved.diagnostics()
    );
}

#[tokio::test]
async fn the_definition_carries_the_belt_the_turn_is_allowed_to_call() {
    // The resolved definition's tool list *is* the turn's allow-list, so a
    // seat whose belt it does not name cannot call it. A definition that
    // names nothing denies everything rather than allowing everything.
    let agent = seat(Some(definition(SEAT, vec!["desk_say", "publish_artifact"])));

    let resolved = catalogue(&agent)
        .resolve(SEAT)
        .await
        .expect("resolution never errors")
        .expect("resolves");

    assert_eq!(resolved.tools, vec!["desk_say", "publish_artifact"]);
}

#[tokio::test]
async fn the_sessions_own_definition_outranks_a_registry_entry_sharing_its_id() {
    // A library host may reuse an id the process registry also knows. Its own
    // definition must win, or the host silently runs someone else's agent.
    let _ = AgentDefinitionRegistry::init_global_builtins();
    let agent = seat(Some(definition("orchestrator", vec!["desk_say"])));

    let resolved = catalogue(&agent)
        .resolve("orchestrator")
        .await
        .expect("resolution never errors")
        .expect("resolves");

    assert_eq!(resolved.name, "Library host seat");
    assert_eq!(resolved.tools, vec!["desk_say"]);
}

#[tokio::test]
async fn without_a_definition_the_same_id_is_still_unknown() {
    // The counterpart, and the reason this route had to exist: nothing else
    // the builder takes can put an id in the catalogue.
    let agent = seat(None);
    let Some(base) = agent.hosted_base.as_ref() else {
        // No catalogue at all, which is the "unknown" this test asserts.
        return;
    };
    assert!(base.session_definition.is_none());

    let catalogue = OpenHumanDefinitionRegistry::new(Arc::clone(&base.definitions))
        .with_config(Arc::clone(&base.config));

    assert!(
        catalogue
            .resolve(SEAT)
            .await
            .expect("resolution never errors")
            .is_none(),
        "the id must not resolve without the caller's definition"
    );
}
