//! Turn lifecycle: running a single interaction, executing tools, and
//! wiring context stats + the sub-agent harness around them.

mod context;
mod core;
mod graph;
mod recall_lanes;
mod session_io;
mod tools;

use tinytools_agent::ParsedToolCall;

use std::borrow::Cow;

/// Built-in direct tools that the orchestrator should call by name, not
/// wrapped in `run_workflow`.
const DIRECT_TOOL_NAMES: &[&str] = &[
    "cron_add",
    "cron_list",
    "cron_remove",
    "cron_update",
    "cron_run",
    "cron_runs",
    "current_time",
];

/// Recovery shim for legacy/wrong-model calls of the form:
/// `run_workflow({workflow_id: "<built-in tool>", inputs: {...}})` (or the
/// pre-rename `run_skill({skill_id: ...})`).
///
/// When this pattern appears, rewrite it into a direct tool call so the turn
/// can proceed without a manual retry.
pub(super) fn normalize_tool_call<'a>(call: &'a ParsedToolCall) -> Cow<'a, ParsedToolCall> {
    if call.name != "run_workflow" && call.name != "run_skill" {
        return Cow::Borrowed(call);
    }
    // Accept either the current `workflow_id` arg or the legacy `skill_id`.
    let Some(target) = call
        .arguments
        .get("workflow_id")
        .or_else(|| call.arguments.get("skill_id"))
        .and_then(|v| v.as_str())
    else {
        return Cow::Borrowed(call);
    };
    if !DIRECT_TOOL_NAMES.contains(&target) {
        return Cow::Borrowed(call);
    }
    let Some(inputs) = call.arguments.get("inputs").and_then(|v| v.as_object()) else {
        return Cow::Borrowed(call);
    };

    log::warn!(
        "[agent_loop] rewrote legacy {}->{} call into direct tool invocation",
        call.name,
        target
    );
    let skill_id = target;
    Cow::Owned(ParsedToolCall {
        name: skill_id.to_string(),
        arguments: serde_json::Value::Object(inputs.clone()),
        id: call.id.clone(),
    })
}

/// Compute the one-shot mid-session connect announcement.
///
/// Given the toolkit slugs currently connected and the set of slugs already
/// announced to the model this session, returns a natural-language note for
/// any genuinely-new slugs (and records them in `announced` so they are never
/// re-announced). Returns `None` when nothing new connected.
///
/// Kept as a free function (no `&self`) so the delta logic is unit-testable
/// without standing up a full `Agent` — see `turn_tests.rs`.
/// Returns the toolkit slugs in `connected` that have not yet been announced
/// this session, marking them announced. Empty when nothing is new.
pub(super) fn newly_connected_slugs(
    connected: &[String],
    announced: &mut std::collections::HashSet<String>,
) -> Vec<String> {
    let newly: Vec<String> = connected
        .iter()
        .filter(|slug| !announced.contains(*slug))
        .cloned()
        .collect();
    for slug in &newly {
        announced.insert(slug.clone());
    }
    newly
}

/// Render the one-shot user-turn note for a set of freshly-connected slugs.
/// Empty input yields `None`.
pub(super) fn integration_announcement_note(slugs: &[String]) -> Option<String> {
    if slugs.is_empty() {
        return None;
    }
    Some(format!(
        "[integration update] These integration(s) connected during this conversation and are available right now: {}. \
Use delegate_to_integrations_agent with the matching toolkit slug to act on them immediately — do not tell the user to reconnect or restart.",
        slugs.join(", ")
    ))
}

/// Render the one-shot user-turn note for MCP server(s) that connected
/// mid-session. The MCP analogue of [`integration_announcement_note`]: the
/// system-prompt `## Connected MCP Servers` block is frozen at turn 1 (KV-cache
/// prefix), so a server connected mid-conversation is surfaced here instead, on
/// the user turn. Empty input yields `None`.
pub(super) fn mcp_announcement_note(servers: &[String]) -> Option<String> {
    if servers.is_empty() {
        return None;
    }
    Some(format!(
        "[MCP update] These MCP server(s) connected during this conversation and are available right now: {}. \
Use the use_mcp_server delegate to act on them immediately — do not tell the user to reconnect or restart.",
        servers.join(", ")
    ))
}

/// One-shot note prepended to the next user turn when skills are installed
/// mid-session. Mirrors [`integration_announcement_note`] for the
/// `## Installed Skills` catalogue: tells the model the freshly-installed
/// skills are usable now (via `run_skill`) so it acts instead of claiming
/// they aren't installed from stale context. Returns `None` when nothing is
/// pending. Rides the user turn (not the system prompt) to keep the KV-cache
/// prefix stable.
pub(super) fn skill_announcement_note(skill_ids: &[String]) -> Option<String> {
    if skill_ids.is_empty() {
        return None;
    }
    Some(format!(
        "[skills update] These skill(s) were installed during this conversation and are available right now: {}. \
They are in your `## Installed Skills` list — run one with `run_skill` immediately; do not tell the user to reinstall or restart.",
        skill_ids.join(", ")
    ))
}

/// One-shot note prepended to the next user turn when skills are uninstalled
/// mid-session. Symmetric to [`skill_announcement_note`]: tells the model the
/// listed skills are no longer present and `run_skill` will fail for them, so
/// it does not attempt to invoke them. Rides the user turn (not the system
/// prompt) to keep the KV-cache prefix stable.
pub(super) fn skill_retraction_note(skill_ids: &[String]) -> Option<String> {
    if skill_ids.is_empty() {
        return None;
    }
    Some(format!(
        "[skills retracted] These skill(s) were uninstalled during this conversation and are no longer available: {}. \
Do not attempt to run them with `run_skill` — they have been removed. Tell the user to reinstall if they want to use them again.",
        skill_ids.join(", ")
    ))
}

/// Every namespace's root summary, under user-resolved per-namespace and total
/// caps. The limits are derived from the active
/// [`crate::config::schema::agent::MemoryContextWindow`]
/// preset by [`crate::config::schema::agent::AgentConfig::resolved_memory_limits`].
///
/// # Why `async` is the whole bridge
///
/// The only caller, `turn::context::fetch_learned_context`, is already an
/// `async fn` and awaits four memory reads immediately above this one, so the
/// driver call is awaited rather than bridged. `block_in_place` — the pattern
/// `session::builder::helpers` uses for a genuinely synchronous caller —
/// panics on a current-thread runtime and would buy nothing here.
///
/// An unresolvable binding, a driver with no tree family, or a failed read all
/// yield no summaries. That is what this returned before as well: the engine's
/// scan swallowed its own failures into an empty vector, and the prompt simply
/// carries no memory block.
pub(super) async fn collect_tree_root_summaries(
    per_namespace_cap: usize,
    total_cap: usize,
) -> Vec<crate::agent::prompts::NamespaceSummary> {
    driver_tree_root_summaries(per_namespace_cap, total_cap)
        .await
        .into_iter()
        .map(
            |(namespace, body, updated_at)| crate::agent::prompts::NamespaceSummary {
                namespace,
                body,
                updated_at,
            },
        )
        .collect()
}

/// The shared subtree's root summaries, read from the guarded driver.
///
/// Returns the same positional `(namespace, body, updated_at)` triple the
/// engine scan did.
async fn driver_tree_root_summaries(
    per_namespace_cap: usize,
    total_cap: usize,
) -> Vec<(String, String, chrono::DateTime<chrono::Utc>)> {
    use crate::memory::api::provider::MemoryProvider;

    let guard = match crate::memory::ops::guard::active_memory_guard().await {
        Ok(guard) => guard,
        Err(error) => {
            log::debug!("[session::turn] tree root summaries: no bound driver ({error})");
            return Vec::new();
        }
    };
    let Some(tree) = guard.as_tree() else {
        log::debug!(
            "[session::turn] tree root summaries: driver '{}' does not serve Tree",
            guard.driver_id()
        );
        return Vec::new();
    };
    match tree
        .root_summaries_with_caps(per_namespace_cap, total_cap)
        .await
    {
        Ok(rows) => rows
            .into_iter()
            .map(|row| (row.namespace, row.body, row.updated_at))
            .collect(),
        Err(error) => {
            log::warn!("[session::turn] tree root summaries: {error}");
            Vec::new()
        }
    }
}

/// Sanitize a learned memory entry before injecting into the system prompt.
/// Strips raw data, limits length, and removes potential secrets.
pub(super) fn sanitize_learned_entry(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    // Truncate to a safe length
    let max_len = 200;
    let sanitized: String = trimmed.chars().take(max_len).collect();
    // Strip anything that looks like a secret/token
    if sanitized.contains("Bearer ")
        || sanitized.contains("sk-")
        || sanitized.contains("ghp_")
        || sanitized.contains("-----BEGIN")
    {
        return "[redacted: potential secret]".to_string();
    }
    sanitized
}

#[cfg(test)]
pub(crate) use super::turn_checkpoint::assistant_message_has_tool_calls;
#[cfg(test)]
pub(crate) use super::types::Agent;
#[cfg(test)]
pub(crate) use crate::agent::prompts::LearnedContextData;
#[cfg(test)]
pub(crate) use anyhow::Result;

#[cfg(test)]
#[path = "../turn_tests.rs"]
mod tests;
