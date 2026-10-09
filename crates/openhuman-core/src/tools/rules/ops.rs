//! Building a session's rule layers and a turn's rule policy.

use std::sync::Arc;

use tinyagents_harness::tool::ToolRulePolicy;
use tinytools::{RuleContext, RuleEffect, ToolRule, ToolRuleSet, ToolRules, glob_matches};

use crate::agent::harness::definition::AgentDefinition;
use crate::config::Config;

/// Whether any entry of a definition tool list matches `name`.
///
/// This is the grammar of every name list on an agent definition
/// (`disallowed_tools`) and the registry's denylist: `*` and `?` globs,
/// ASCII case-insensitive, so the historical exact names and trailing-`*`
/// prefixes keep matching what they did. It is the one implementation; the
/// session builder, sub-agent tool preparation, parallel staging and the
/// hosted definition projection all call it.
#[must_use]
pub fn glob_list_matches(patterns: &[String], name: &str) -> bool {
    patterns.iter().any(|pattern| glob_matches(pattern, name))
}

/// The rule layer an agent definition contributes: its `tool_rules`, plus a
/// `deny` for its `disallowed_tools`, so a denied tool is refused on every
/// surface — including `tool_search` and a call by a guessed name — not only
/// removed from the visible belt. `None` when the definition restricts
/// nothing.
#[must_use]
pub fn agent_rule_layer(def: &AgentDefinition) -> Option<ToolRules> {
    let mut layer = def.tool_rules.clone().unwrap_or_default();
    if !def.disallowed_tools.is_empty() {
        layer.rules.push(
            ToolRule::names(RuleEffect::Deny, def.disallowed_tools.iter().cloned())
                .with_id("disallowed_tools"),
        );
    }
    if layer.is_permissive() {
        return None;
    }
    if layer.name.is_none() {
        layer.name = Some(format!("agent:{}", def.id));
    }
    Some(layer)
}

/// The rule layers a session carries: the operator's `[tool_rules]`, then
/// the definition's layer. Either may be absent.
#[must_use]
pub fn session_rule_set(config: Option<&Config>, def: Option<&AgentDefinition>) -> ToolRuleSet {
    let mut set = ToolRuleSet::new();
    if let Some(config) = config {
        let mut layer = config.tool_rules.clone();
        if layer.name.is_none() {
            layer.name = Some("config".to_string());
        }
        set.push(layer);
    }
    if let Some(layer) = def.and_then(agent_rule_layer) {
        set.push(layer);
    }
    tracing::debug!(
        agent = def.map(|d| d.id.as_str()),
        layers = set.layers.len(),
        "[tool_rules] session rule set composed"
    );
    set
}

/// The context `when` conditions match: `channel`, `agent`, `origin`. Absent
/// or blank values are left out, so a condition on them does not match.
#[must_use]
pub fn rule_context(channel: Option<&str>, agent: Option<&str>, origin: Option<&str>) -> RuleContext {
    let mut context = RuleContext::new();
    for (key, value) in [("channel", channel), ("agent", agent), ("origin", origin)] {
        if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
            context = context.with(key, value);
        }
    }
    context
}

/// The harness policy for one turn: `rules` evaluated in `context`.
#[must_use]
pub fn turn_rule_policy(rules: Arc<ToolRuleSet>, context: RuleContext) -> ToolRulePolicy {
    ToolRulePolicy { rules, context }
}
