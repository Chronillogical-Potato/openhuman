//! Host-only sessions: the host's belt is the whole belt.
//!
//! [`HostTurnTools`](super::HostTurnTools) adds a host's tools *beside* the
//! config-derived registry; a withheld config tool stays registered. That is
//! the right default for an embedder extending an agent, and the wrong one for
//! a host that hands an agent read-only tools over untrusted input (a PR-review
//! bot reading a diff) and must be certain the model can never act.
//!
//! [`OpenHumanSessionHost::from_config_host_only`] builds that session. Three
//! independent layers, so no single mistake re-opens the surface:
//!
//! 1. **Registry.** No config-derived tool is constructed and no delegation
//!    tool is synthesised; the belt is the host's tools and nothing else.
//! 2. **Policy.** [`HostOnlyToolPolicy`] denies every call whose name is not
//!    one of the host's, before the host's own gate is consulted, so even a
//!    model that names a built-in tool is refused.
//! 3. **Definition.** The session runs a narrowed copy of the definition
//!    ([`host_only_definition`]): no sub-agents (so the per-turn delegation
//!    refresh has nothing to add), no deferred tools, read-only sandbox, and
//!    no memory context.

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;

use crate::agent::harness::definition::{AgentDefinition, SandboxMode, ToolScope};
use crate::agent::tool_policy::{ToolPolicy, ToolPolicyDecision, ToolPolicyRequest};
use crate::agent::OpenHumanSessionHost;
use crate::config::Config;
use anyhow::Result;

/// Deny-by-default gate for a host-only session.
///
/// A name outside `allowed` is refused outright. A name inside it is then put
/// to the host's own gate, when the host supplied one, so host-only narrows a
/// host's policy and never widens it.
pub struct HostOnlyToolPolicy {
    allowed: HashSet<String>,
    host: Option<Arc<dyn ToolPolicy>>,
}

impl HostOnlyToolPolicy {
    /// Admit exactly `allowed`, subject to `host`'s own decision.
    #[must_use]
    pub fn new(allowed: HashSet<String>, host: Option<Arc<dyn ToolPolicy>>) -> Self {
        Self { allowed, host }
    }
}

#[async_trait]
impl ToolPolicy for HostOnlyToolPolicy {
    fn name(&self) -> &str {
        "host_only"
    }

    async fn check(&self, request: &ToolPolicyRequest) -> ToolPolicyDecision {
        if !self.allowed.contains(&request.tool_name) {
            log::debug!(
                "[agent::host_only] denied tool outside the host belt: {}",
                request.tool_name
            );
            return ToolPolicyDecision::deny(format!(
                "tool `{}` is not available to this agent",
                request.tool_name
            ));
        }
        match &self.host {
            Some(host) => host.check(request).await,
            None => ToolPolicyDecision::Allow,
        }
    }
}

/// The definition a host-only session runs under: `definition` with every
/// route to a tool the host did not supply removed.
pub(super) fn host_only_definition(definition: &AgentDefinition) -> AgentDefinition {
    let mut def = definition.clone();
    // An empty named belt is zero tools; the host's names join it as scope
    // additions when the belt is merged.
    def.tools = ToolScope::Named(Vec::new());
    def.extra_tools.clear();
    def.deferred_tools.clear();
    def.skill_filter = None;
    // Without sub-agents nothing is synthesised at build time, and
    // `refresh_delegation_tools` returns before adding any later.
    def.subagents.clear();
    def.sandbox_mode = SandboxMode::ReadOnly;
    def.omit_memory_context = true;
    def
}

impl OpenHumanSessionHost {
    /// A session whose whole tool belt is what `host` supplies.
    ///
    /// See the module docs for the three layers this applies. `host` may be
    /// `None`, which is a session with no tools at all. The `definition`'s
    /// tool scope, sub-agents and sandbox are ignored: host-only overrides
    /// them.
    ///
    /// # Errors
    ///
    /// As [`OpenHumanSessionHost::from_config_with_definition`].
    pub fn from_config_host_only(
        config: &Config,
        definition: &AgentDefinition,
        host: Option<&super::HostTools>,
        session_id: Option<&str>,
    ) -> Result<Self> {
        let definition = host_only_definition(definition);
        OpenHumanSessionHost::build_session_agent_inner(
            config,
            &definition.id,
            Some(&definition),
            super::factory::SessionBelt::HostOnly,
            host,
            session_id,
        )
    }
}
