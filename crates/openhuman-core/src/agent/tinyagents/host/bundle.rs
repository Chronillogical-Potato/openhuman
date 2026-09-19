//! The one factory that assembles OpenHuman's TinyAgents host capabilities.
//!
//! Keeping this construction in one place makes the boundary auditable: every
//! concrete adapter receives values from the same host session and explicit
//! [`OpenHumanRunContext`], and the generic runtime receives only its canonical
//! [`tinyagents_harness::host::HostCapabilities`] bundle.

use std::sync::Arc;

use tinyagents_harness::host::{
    AgentMemory, BudgetGate, ContextComposer, ExperienceStore, HostCapabilities, LearningSink,
    ModelResolver, ProgressSink, SecurityGate, ToolOutcomeClassifier,
};

use crate::agent::harness::definition::AgentDefinitionRegistry;
use crate::agent::hooks::PostTurnHook;
use crate::config::Config;
use crate::memory::Memory;
use crate::security::policy::SecurityPolicy;
use crate::tools::agent_policy::ToolPolicySession;
use tinytools::Tool;

use super::{
    OpenHumanAgentMemory, OpenHumanBudgetGate, OpenHumanContextComposer,
    OpenHumanDefinitionRegistry, OpenHumanExperienceStore, OpenHumanLearningSink,
    OpenHumanModelResolver, OpenHumanProgressSink, OpenHumanRunContext, OpenHumanSecurityGate,
    OpenHumanToolOutcomeClassifier,
};

/// Runtime-owned inputs required to build every OpenHuman host capability.
///
/// This is deliberately an input value, rather than a global lookup. The
/// caller obtains it while building a session and passes it alongside the
/// matching [`OpenHumanRunContext`].
pub struct OpenHumanHostBundleInputs {
    pub config: Arc<Config>,
    pub definitions: Arc<AgentDefinitionRegistry>,
    pub security_policy: Arc<SecurityPolicy>,
    pub tool_sets: Vec<Arc<Vec<Box<dyn Tool>>>>,
    pub tool_policy: Option<Arc<ToolPolicySession>>,
    pub memory: Arc<dyn Memory>,
    pub shared_experience_memory: Option<Arc<dyn Memory>>,
    pub post_turn_hooks: Vec<Arc<dyn PostTurnHook>>,
}

/// Process/session-owned dependencies shared by hosted invocations.
///
/// This deliberately excludes mutable turn authority: tools, tool-policy
/// sessions, progress and the concrete capability bundle are created for each
/// [`OpenHumanHostInvocationInputs`]. Keeping the split visible prevents a
/// concurrent turn from replacing another turn's security or tool surface.
pub struct OpenHumanHostBase {
    pub config: Arc<Config>,
    pub definitions: Arc<AgentDefinitionRegistry>,
    pub security_policy: Arc<SecurityPolicy>,
    pub memory: Arc<dyn Memory>,
    pub shared_experience_memory: Option<Arc<dyn Memory>>,
    pub post_turn_hooks: Vec<Arc<dyn PostTurnHook>>,
}

/// Explicit inputs which vary for every hosted agent invocation.
pub struct OpenHumanHostInvocationInputs {
    pub base: Arc<OpenHumanHostBase>,
    pub tool_sets: Vec<Arc<Vec<Box<dyn Tool>>>>,
    pub tool_policy: Option<Arc<ToolPolicySession>>,
    /// The current turn's already-selected model route set.
    pub model_resolver: Option<Arc<dyn ModelResolver<()>>>,
}

/// OpenHuman's ten concrete capability adapters plus their erased crate bundle.
///
/// Typed handles make it possible to verify wiring without downcasting trait
/// objects. The runtime consumes [`Self::capabilities`]; the typed fields exist
/// solely to preserve the host boundary and to support focused wiring tests.
pub struct OpenHumanHostBundle {
    pub capabilities: HostCapabilities<()>,
    pub context: Arc<OpenHumanContextComposer>,
    pub definitions: Arc<OpenHumanDefinitionRegistry>,
    pub security: Arc<OpenHumanSecurityGate>,
    pub models: Arc<OpenHumanModelResolver>,
    pub memory: Arc<OpenHumanAgentMemory>,
    pub budget: Arc<OpenHumanBudgetGate>,
    pub progress: Arc<OpenHumanProgressSink>,
    pub learning: Arc<OpenHumanLearningSink>,
    pub tool_outcomes: Arc<OpenHumanToolOutcomeClassifier>,
    pub experience: Arc<OpenHumanExperienceStore>,
}

/// Builds the full OpenHuman host bundle for an explicit turn.
pub struct OpenHumanHostBundleFactory;

impl OpenHumanHostBundleFactory {
    /// Builds one full capability bundle from immutable host base dependencies
    /// and this invocation's tool/security authority.
    pub fn build_for_invocation(
        inputs: OpenHumanHostInvocationInputs,
        turn: &OpenHumanRunContext,
    ) -> OpenHumanHostBundle {
        let mut bundle = Self::build(
            OpenHumanHostBundleInputs {
                config: inputs.base.config.clone(),
                definitions: inputs.base.definitions.clone(),
                security_policy: inputs.base.security_policy.clone(),
                tool_sets: inputs.tool_sets,
                tool_policy: inputs.tool_policy,
                memory: inputs.base.memory.clone(),
                shared_experience_memory: inputs.base.shared_experience_memory.clone(),
                post_turn_hooks: inputs.base.post_turn_hooks.clone(),
            },
            turn,
        );
        if let Some(model_resolver) = inputs.model_resolver {
            bundle.capabilities.models = model_resolver;
        }
        bundle
    }

    /// Constructs all ten concrete adapters from a single session input set.
    ///
    /// The run context supplies per-turn state, while `inputs` supplies durable
    /// session/runtime dependencies. No adapter is optional for OpenHuman. An
    /// unobserved context gets an unconsumed bounded sink, preserving the
    /// concrete progress seam without discovering state through a task-local.
    pub fn build(
        inputs: OpenHumanHostBundleInputs,
        turn: &OpenHumanRunContext,
    ) -> OpenHumanHostBundle {
        let context = Arc::new(OpenHumanContextComposer::new(Arc::clone(&inputs.config)));
        let definitions = Arc::new(
            OpenHumanDefinitionRegistry::new(inputs.definitions)
                .with_config(Arc::clone(&inputs.config)),
        );
        let mut security = OpenHumanSecurityGate::new(inputs.security_policy, inputs.tool_sets);
        if let Some(policy) = inputs.tool_policy {
            security = security.with_tool_policy(policy);
        }
        let security = Arc::new(security);
        let models = Arc::new(OpenHumanModelResolver::new(Arc::clone(&inputs.config)));
        let memory = Arc::new(OpenHumanAgentMemory::new(Arc::clone(&inputs.memory)));
        let budget = Arc::new(OpenHumanBudgetGate::new(Arc::clone(&inputs.config)));
        let progress_tx = turn
            .progress
            .clone()
            .unwrap_or_else(|| tokio::sync::mpsc::channel(1).0);
        let progress = Arc::new(OpenHumanProgressSink::new(progress_tx));
        let learning = Arc::new(OpenHumanLearningSink::new(inputs.post_turn_hooks));
        let tool_outcomes = Arc::new(OpenHumanToolOutcomeClassifier::new());
        let experience = Arc::new(
            OpenHumanExperienceStore::new(inputs.memory)
                .with_shared_recall_memory(inputs.shared_experience_memory),
        );

        let capabilities = HostCapabilities::new(
            context.clone() as Arc<dyn ContextComposer>,
            definitions.clone(),
            security.clone() as Arc<dyn SecurityGate>,
            models.clone() as Arc<dyn ModelResolver<()>>,
        )
        .with_memory(memory.clone() as Arc<dyn AgentMemory>)
        .with_budget(budget.clone() as Arc<dyn BudgetGate>)
        .with_progress(progress.clone() as Arc<dyn ProgressSink>)
        .with_learning(learning.clone() as Arc<dyn LearningSink>)
        .with_tool_outcomes(tool_outcomes.clone() as Arc<dyn ToolOutcomeClassifier>)
        .with_experience(experience.clone() as Arc<dyn ExperienceStore>);

        OpenHumanHostBundle {
            capabilities,
            context,
            definitions,
            security,
            models,
            memory,
            budget,
            progress,
            learning,
            tool_outcomes,
            experience,
        }
    }
}

#[cfg(test)]
#[path = "bundle_tests.rs"]
mod tests;
