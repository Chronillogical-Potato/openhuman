//! What a context derived for one agent owns: its policy, approval switch,
//! sub-agent catalogue and state slots.

use super::*;

#[derive(Clone, Default)]
pub(super) struct AgentParts {
    /// `None` for booted contexts, which read the process live policy.
    policy: Option<Arc<crate::security::SecurityPolicy>>,
    approvals_disabled: bool,
    /// `None` resolves through the process registry.
    definitions: Option<Arc<crate::agent::harness::definition::AgentDefinitionRegistry>>,
    /// Shared by every turn context derived from the same agent context.
    state: Arc<super::super::agent_scope::AgentScopedState>,
}

impl AgentParts {
    /// The parts of a context derived from one owning `self` with `overlay`.
    /// An overlay that names an agent gets state slots of its own.
    pub(super) fn derive(&self, overlay: &mut ContextOverlay) -> Self {
        Self {
            policy: overlay.agent_policy.take().or_else(|| self.policy.clone()),
            approvals_disabled: overlay.approvals_disabled || self.approvals_disabled,
            definitions: overlay
                .definitions
                .take()
                .or_else(|| self.definitions.clone()),
            state: if overlay.session_agent.is_some() {
                Default::default()
            } else {
                Arc::clone(&self.state)
            },
        }
    }
}

impl CoreContext {
    /// The security policy of the agent this context was derived for.
    pub fn agent_policy(&self) -> Option<Arc<crate::security::SecurityPolicy>> {
        self.agent.policy.clone()
    }

    /// [`agent_policy`](Self::agent_policy) of the ambient context.
    pub fn current_agent_policy() -> Option<Arc<crate::security::SecurityPolicy>> {
        Self::current().and_then(|ctx| ctx.agent.policy.clone())
    }

    /// Whether the interactive approval gate is off for this context's agent.
    pub fn approvals_disabled(&self) -> bool {
        self.agent.approvals_disabled
    }

    /// [`approvals_disabled`](Self::approvals_disabled) of the ambient context.
    pub fn current_approvals_disabled() -> bool {
        Self::current().is_some_and(|ctx| ctx.agent.approvals_disabled)
    }

    /// The sub-agent catalogue of the agent this context was derived for.
    pub fn definitions(
        &self,
    ) -> Option<Arc<crate::agent::harness::definition::AgentDefinitionRegistry>> {
        self.agent.definitions.clone()
    }

    /// The state slots this context owns.
    pub fn agent_state(&self) -> &super::super::agent_scope::AgentScopedState {
        &self.agent.state
    }
}
