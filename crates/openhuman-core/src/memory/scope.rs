//! Whose memory a turn reads and writes.
//!
//! Memory follows TinyMemory's standard layout (`tinymemory_tools::MemoryLayout`):
//! one tree per tenant, below a root —
//!
//! ```text
//! <root>                      shared learnings; holistic recall reads it all
//! ├── source:<kind>           the brain: synced documents, no agent id
//! └── agent:<memory agent>    one agent's conversations, a turn per item
//! ```
//!
//! A [`MemoryIdentity`] is who is acting: an agent definition and, for a team
//! member, its team. The host scopes one around every agent turn
//! ([`within_agent`], [`within`]); memory resolves it against the config of
//! whoever reads or writes ([`MemoryIdentity::resolve`]) into a
//! [`ResolvedIdentity`]: the layout root and the memory agent id.
//!
//! Resolution, first match wins:
//!
//! | | memory agent id | layout root |
//! | --- | --- | --- |
//! | 1. host binding | `[memory] agent_id` | `[memory] root` |
//! | 2. definition pin | `[memory.agents.<definition>] agent_id` | `[memory.agents.<definition>] root` |
//! | 3. team member | the definition id | `team:<team>` |
//! | 4. default | the definition id, else [`DEFAULT_AGENT_ID`] | the default root |
//!
//! The host binding is how a coordinating host (OpenCompany, an embedder via
//! `openhuman_embed::AgentSpec::memory`) gives each OpenHuman agent it runs a
//! memory of its own: it derives a per-agent config with those two keys set.
//! Everything that agent runs — sub-agents included — then acts as that one
//! memory agent.
//!
//! The identity is never taken from model arguments.

use std::future::Future;

use tinymemory_api::{Namespace, Segment, SegmentKind};
use tinymemory_tools::MemoryLayout;

use crate::config::Config;

/// The memory agent id of work no agent is running (RPC, sync jobs, the UI).
pub const DEFAULT_AGENT_ID: &str = "assistant";

tokio::task_local! {
    static CURRENT: MemoryIdentity;
}

/// Who is acting on memory: the agent definition and its team. Scoping one
/// needs no config; what it maps to is resolved against the config of
/// whoever reads or writes ([`Self::resolve`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryIdentity {
    /// The agent definition id; `None` outside any agent (RPC, sync jobs).
    pub agent_id: Option<String>,
    /// The team the agent works in.
    pub team: Option<String>,
}

/// An identity resolved against a config: where its memory lives.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResolvedIdentity {
    /// The layout below the identity's root.
    pub layout: MemoryLayout,
    /// The memory agent id its turns are logged under.
    pub agent_id: String,
    /// Whether its turns get a context pack.
    pub recall: bool,
}

impl ResolvedIdentity {
    /// The layout root.
    #[must_use]
    pub fn root(&self) -> &Namespace {
        self.layout.root()
    }
}

impl MemoryIdentity {
    /// No agent: work the host does on its own behalf.
    #[must_use]
    pub fn root() -> Self {
        Self::default()
    }

    /// A top-level agent.
    #[must_use]
    pub fn agent(agent_id: &str) -> Self {
        Self {
            agent_id: non_blank(agent_id),
            team: None,
        }
    }

    /// `agent_id` as a member of `team`.
    #[must_use]
    pub fn team_member(team: &str, agent_id: &str) -> Self {
        Self {
            agent_id: non_blank(agent_id),
            team: non_blank(team),
        }
    }

    /// `agent_id` run by this agent: a sub-agent, in the same team.
    #[must_use]
    pub fn child(&self, agent_id: &str) -> Self {
        Self {
            agent_id: non_blank(agent_id),
            team: self.team.clone(),
        }
    }

    /// Where this identity's memory lives under `config`. See the module
    /// docs for the resolution order.
    #[must_use]
    pub fn resolve(&self, config: &Config) -> ResolvedIdentity {
        let memory = &config.memory;
        let pinned = self
            .agent_id
            .as_deref()
            .and_then(|definition| memory.agents.get(definition));
        let agent_id = non_blank_opt(memory.agent_id.as_deref())
            .or_else(|| non_blank_opt(pinned.and_then(|pin| pin.agent_id.as_deref())))
            .or_else(|| self.agent_id.clone())
            .unwrap_or_else(|| DEFAULT_AGENT_ID.to_string());
        let root = parse_root(memory.root.as_deref())
            .or_else(|| parse_root(pinned.and_then(|pin| pin.root.as_deref())))
            .or_else(|| self.team.as_deref().map(team_root))
            .unwrap_or(Namespace::ROOT);
        let layout = MemoryLayout::new(root).unwrap_or_else(|error| {
            tracing::warn!(%error, "[memory:scope] root too deep; using the default root");
            MemoryLayout::default()
        });
        let recall = pinned
            .and_then(|pin| pin.recall)
            .unwrap_or(memory.recall.enabled);
        ResolvedIdentity {
            layout,
            agent_id,
            recall,
        }
    }
}

/// The layout `team`'s members share.
fn team_root(team: &str) -> Namespace {
    Namespace::ROOT
        .child(Segment::sanitized(SegmentKind::Team, team))
        .unwrap_or(Namespace::ROOT)
}

/// Checks that `root` names a usable layout root (`team:acme`,
/// `project:q4/team:ops`), as a host binding is set.
///
/// # Errors
///
/// Why it is not one.
pub fn validate_root(root: &str) -> Result<(), String> {
    let root: Namespace = root
        .trim()
        .parse()
        .map_err(|error: tinymemory_api::Error| error.to_string())?;
    MemoryLayout::new(root)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// A configured root; an invalid one is logged and ignored.
fn parse_root(raw: Option<&str>) -> Option<Namespace> {
    let raw = raw?.trim();
    if raw.is_empty() {
        return None;
    }
    match raw.parse::<Namespace>() {
        Ok(root) => Some(root),
        Err(error) => {
            tracing::warn!(root = raw, %error, "[memory:scope] ignoring an invalid configured root");
            None
        }
    }
}

fn non_blank(value: &str) -> Option<String> {
    non_blank_opt(Some(value))
}

fn non_blank_opt(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// The identity in scope, or the root identity outside any agent, resolved
/// against `config`.
#[must_use]
pub fn resolve_current(config: &Config) -> ResolvedIdentity {
    current().unwrap_or_default().resolve(config)
}

/// Runs `fut` as `identity`.
pub async fn within<F: Future>(identity: MemoryIdentity, fut: F) -> F::Output {
    tracing::debug!(
        agent_id = identity.agent_id.as_deref().unwrap_or("-"),
        team = identity.team.as_deref().unwrap_or("-"),
        "[memory:scope] acting as"
    );
    CURRENT.scope(identity, fut).await
}

/// The identity scoped around the running task, if any.
#[must_use]
pub fn current() -> Option<MemoryIdentity> {
    CURRENT.try_with(Clone::clone).ok()
}

/// Runs a turn of `agent_id`: a turn of the agent already in scope keeps its
/// identity; another agent runs as a child of it (same team).
pub async fn within_agent<F: Future>(agent_id: &str, fut: F) -> F::Output {
    let identity = match current() {
        Some(outer) if outer.agent_id.as_deref() == Some(agent_id) => outer,
        Some(outer) => outer.child(agent_id),
        None => MemoryIdentity::agent(agent_id),
    };
    within(identity, fut).await
}

#[cfg(test)]
#[path = "scope_tests.rs"]
mod tests;
