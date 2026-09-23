use super::*;

use std::sync::Arc;

use tinyagents_definition::DefinitionRegistry;
use tinyagents_harness::host::{
    AgentMemory, BudgetGate, ContextComposer, ExperienceStore, LearningSink, ModelResolver,
    ProgressSink, SecurityGate, ToolOutcomeClassifier,
};

fn inputs() -> (OpenHumanHostBundleInputs, OpenHumanRunContext) {
    let (progress, _receiver) = tokio::sync::mpsc::channel(4);
    let memory: Arc<dyn Memory> =
        Arc::new(crate::memory::tool_memory::test_helpers::MockMemory::default());
    let mut turn = OpenHumanRunContext::new();
    turn.thread_id = Some("thread-for-bundle".to_string());
    turn.progress = Some(progress);
    (
        OpenHumanHostBundleInputs {
            config: Arc::new(Config::default()),
            definitions: Arc::new(AgentDefinitionRegistry::builtins_only()),
            security_policy: Arc::new(SecurityPolicy::default()),
            tool_sets: Vec::new(),
            tool_policy: None,
            memory,
            post_turn_hooks: Vec::new(),
            session_definition: None,
        },
        turn,
    )
}

#[test]
fn factory_records_all_ten_concrete_adapters_in_one_bundle() {
    let (inputs, turn) = inputs();
    let host = OpenHumanHostBundleFactory::build(inputs, &turn);

    let context: Arc<dyn ContextComposer> = host.context.clone();
    let definitions: Arc<dyn DefinitionRegistry> = host.definitions.clone();
    let security: Arc<dyn SecurityGate> = host.security.clone();
    let models: Arc<dyn ModelResolver<()>> = host.models.clone();
    let memory: Arc<dyn AgentMemory> = host.memory.clone();
    let budget: Arc<dyn BudgetGate> = host.budget.clone();
    let progress: Arc<dyn ProgressSink> = host.progress.clone();
    let learning: Arc<dyn LearningSink> = host.learning.clone();
    let outcomes: Arc<dyn ToolOutcomeClassifier> = host.tool_outcomes.clone();
    let experience: Arc<dyn ExperienceStore> = host.experience.clone();

    assert!(Arc::ptr_eq(&context, &host.capabilities.context));
    assert!(Arc::ptr_eq(&definitions, &host.capabilities.definitions));
    assert!(Arc::ptr_eq(&security, &host.capabilities.security));
    assert!(Arc::ptr_eq(&models, &host.capabilities.models));
    assert!(Arc::ptr_eq(
        &memory,
        host.capabilities.memory.as_ref().expect("memory")
    ));
    assert!(Arc::ptr_eq(
        &budget,
        host.capabilities.budget.as_ref().expect("budget")
    ));
    assert!(Arc::ptr_eq(
        &progress,
        host.capabilities.progress.as_ref().expect("progress")
    ));
    assert!(Arc::ptr_eq(
        &learning,
        host.capabilities.learning.as_ref().expect("learning")
    ));
    assert!(Arc::ptr_eq(
        &outcomes,
        host.capabilities
            .tool_outcomes
            .as_ref()
            .expect("tool outcomes"),
    ));
    assert!(Arc::ptr_eq(
        &experience,
        host.capabilities.experience.as_ref().expect("experience"),
    ));
}
