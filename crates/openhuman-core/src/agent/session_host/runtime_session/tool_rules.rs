//! Tool rules for a session's turns (`crate::tools::rules`).
//!
//! A session composes its rule layers once — the operator's `[tool_rules]`
//! and its agent definition's layer — and every turn evaluates them in that
//! turn's own context (channel, agent, origin), installed on
//! `OpenHumanRunContext::tool_rules` for the harness gate.

use std::sync::Arc;

use super::{OpenHumanSessionHost, OpenHumanTurnPrelude};
use crate::agent::tinyagents::host::OpenHumanRunContext;
use crate::agent::turn_origin::AgentTurnOrigin;
use tinyagents_harness::tool::ToolRulePolicy;

impl OpenHumanSessionHost {
    /// The session's rule layers, from its config and resolved definition.
    pub(super) fn session_tool_rules(&self) -> Arc<tinytools::ToolRuleSet> {
        Arc::new(crate::tools::rules::session_rule_set(
            self.runtime_config.as_deref(),
            self.resolved_definition().as_deref(),
        ))
    }
}

impl OpenHumanTurnPrelude {
    /// This turn's rule policy: the session's layers in the turn's context.
    /// `None` when the session's rules restrict nothing.
    pub(super) fn turn_tool_rules(
        &self,
        channel: &str,
        run_context: &OpenHumanRunContext,
    ) -> Option<Arc<ToolRulePolicy>> {
        if self.tool_rules.is_permissive() {
            return None;
        }
        let origin = run_context.origin.as_ref().map(AgentTurnOrigin::class);
        let context = crate::tools::rules::rule_context(
            Some(channel),
            Some(&self.agent_definition_id),
            origin.as_deref(),
        );
        Some(Arc::new(crate::tools::rules::turn_rule_policy(
            self.tool_rules.clone(),
            context,
        )))
    }
}

#[cfg(test)]
#[path = "tool_rules_tests.rs"]
mod tests;
