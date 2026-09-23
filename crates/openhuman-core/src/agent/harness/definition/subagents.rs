//! `subagents` allowlist entries: agent ids and the `{ skills = "*" }`
//! wildcard, plus the lenient deserializer that accepts both TOML shapes.

use serde::{Deserialize, Deserializer, Serialize};

/// One entry in [`super::AgentDefinition::subagents`]. Parses from TOML as either
/// a bare string (agent id) or an inline table (`{ skills = "*" }`) thanks
/// to `#[serde(untagged)]`.
///
/// # TOML shapes
///
/// ```toml
/// [subagents]
/// allowlist = [
///     "researcher",            # AgentId("researcher")
///     "code_executor",         # AgentId("code_executor")
///     { skills = "*" },        # Skills { pattern: "*" }
/// ]
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum SubagentEntry {
    /// Delegate to a specific built-in or custom agent by id.
    AgentId(String),
    /// Expand at build time to the connected Composio toolkits' actions as
    /// `Deferred` tools — off the wire, found through the harness's
    /// `tool_search`, and called directly by this agent. No sub-agent is
    /// reachable through this entry: it widens the searchable catalogue,
    /// not the spawnable set.
    Skills(SkillsWildcard),
}

/// The `{ skills = "*" }` inline table in a `subagents` list.
///
/// Today only `"*"` is meaningful (expand to every connected toolkit).
/// Future: a `Vec<String>` variant to restrict expansion to specific
/// toolkit slugs (e.g. `{ skills = ["gmail", "notion"] }`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillsWildcard {
    /// Glob / wildcard pattern. Only `"*"` is currently supported.
    pub skills: String,
}

pub(super) fn deserialize_subagent_entries<'de, D>(
    deserializer: D,
) -> Result<Vec<SubagentEntry>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wire {
        Section { allowlist: Vec<SubagentEntry> },
        LegacyList(Vec<SubagentEntry>),
    }

    match Option::<Wire>::deserialize(deserializer)? {
        Some(Wire::Section { allowlist }) => Ok(allowlist),
        Some(Wire::LegacyList(entries)) => Ok(entries),
        None => Ok(Vec::new()),
    }
}

impl SkillsWildcard {
    /// True when this wildcard should expand to every connected toolkit.
    pub fn matches_all(&self) -> bool {
        self.skills == "*"
    }
}
