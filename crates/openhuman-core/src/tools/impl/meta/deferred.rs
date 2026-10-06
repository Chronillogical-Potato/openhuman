//! The host's half of [`ToolExposure::Deferred`](tinytools::ToolExposure):
//! which registered tools leave the wire, so the harness can advertise
//! `tool_search` in their place.
//!
//! # Why this exists
//!
//! Tool schemas are a fixed cost paid on every request. Measured on the
//! orchestrator with an empty workspace, they were 45,199 bytes against 33,808
//! bytes of system prompt; on a signed-in workspace with Composio connected
//! they reached ~112 KB. Most of that is tools the model reaches for on a
//! handful of turns a week.
//!
//! `Deferred` takes such a tool off the wire without removing the capability.
//! The tinyagents harness owns the other half of that bargain — the intrinsic
//! `tool_search` bridge, the BM25 catalogue, and the pluggable
//! ranker a decision model plugs into (`tool::discover`). This host used to
//! register a `tool_search` of its own, which the harness honoured over its
//! intrinsic; that tool could *find* a deferred tool but the turn allowlist
//! never registered it, so the find was unusable. Now the host only decides
//! what is deferred and hands the harness both sets: the advertised names as
//! the wire surface and the deferred names as still-callable registrations.
//!
//! # Why not the toolpack mechanism
//!
//! Packs answer a different question. A pack is a *group* withheld by a config
//! posture, recovered through `use_skill`, and its membership is compiled in
//! precisely so config cannot move a dangerous tool out of the reviewed
//! surface. That is the right shape for compressing a belt an agent owns.
//!
//! Deferral is per-tool and is a property of the tool: `stock_quote` is rarely
//! needed whoever is running the host. The two compose — a deferred tool inside
//! a withheld pack is simply absent twice — and neither can widen the surface,
//! because both only ever subtract from a set the belt and the security policy
//! already decided.

use std::collections::HashSet;

use tinytools::{Tool, ToolExposure};

/// The name an agent lists in `[tools] named` to opt a hand-written belt into
/// discovery. It is not a registered tool: the harness advertises its own
/// intrinsic `tool_search` whenever a run has a deferred tool, so the name in
/// a belt is a request for that, and the builder strips it from the allowlist.
pub const TOOL_SEARCH_NAME: &str = "tool_search";

/// Remove every [`ToolExposure::Deferred`] and [`ToolExposure::Hidden`] tool
/// from an agent's advertised set, returning the names of the deferred ones so
/// the caller can keep them registered.
///
/// Hidden tools are dropped and **not** returned: they are not searchable
/// either, by definition.
pub fn strip_deferred_from_visible(
    visible: &mut HashSet<String>,
    tools: &[Box<dyn Tool>],
) -> HashSet<String> {
    let mut deferred = HashSet::new();
    for tool in tools {
        let name = tool.name();
        if !visible.contains(name) {
            continue;
        }
        match tool.exposure() {
            ToolExposure::Direct => {}
            ToolExposure::Deferred => {
                visible.remove(name);
                deferred.insert(name.to_string());
            }
            ToolExposure::Hidden => {
                visible.remove(name);
            }
        }
    }
    deferred
}

/// Every [`ToolExposure::Deferred`] tool in `tools`, by name — the catalogue a
/// belt that opted into discovery can reach whether or not it named them.
pub fn deferred_tool_names(tools: &[Box<dyn Tool>]) -> HashSet<String> {
    tools
        .iter()
        .filter(|tool| tool.exposure() == ToolExposure::Deferred)
        .map(|tool| tool.name().to_string())
        .collect()
}

/// The deferred set of a belt that opted into discovery: every
/// [`ToolExposure::Deferred`] tool in `durable` and `synthesized`, plus the
/// `Direct` tools its agent definition lists in `deferred_tools`.
///
/// Exposure is a property of the tool and applies to every agent; a
/// definition's `deferred_tools` is the per-agent half. It takes a tool that
/// other belts keep advertised (`file_read` for a coding worker) off this
/// agent's wire only, where it stays registered, searchable through
/// `tool_search`, and callable by its own name. A requested name that is not
/// registered, or is `Hidden`, is ignored: deferral only ever subtracts.
pub fn deferred_set(
    durable: &[Box<dyn Tool>],
    synthesized: &[Box<dyn Tool>],
    requested: &[String],
) -> HashSet<String> {
    let mut deferred = deferred_tool_names(durable);
    deferred.extend(deferred_tool_names(synthesized));
    for tool in durable.iter().chain(synthesized.iter()) {
        if tool.exposure() == ToolExposure::Direct && requested.iter().any(|r| r == tool.name()) {
            deferred.insert(tool.name().to_string());
        }
    }
    deferred
}

#[cfg(test)]
#[path = "deferred_tests.rs"]
mod tests;
