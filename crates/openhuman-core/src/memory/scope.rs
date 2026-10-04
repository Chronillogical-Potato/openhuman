//! Whose memory a read or write belongs to: agent namespaces.
//!
//! Memory is a tree of nodes (`tinymemory::Namespace`). The root holds what
//! every agent shares — the main chat agent's learnings, synced documents,
//! imported and backfilled history. Every other agent has its own node,
//! `agent:<id>`; a sub-agent's node nests under the agent that spawned it
//! (`agent:researcher/agent:scout`), and a team member's under its team
//! (`team:<id>/agent:<id>`). Inside each node, learnings, documents and
//! conversations are kept as separate scopes by the engine.
//!
//! A [`MemoryIdentity`] is the acting agent, the agents that spawned it and
//! its team. The host scopes one around every agent turn ([`within_agent`]):
//! the session host and channel dispatch for top-level turns, the sub-agent
//! runner for children (nested automatically), the team runtime for members.
//! Memory then reads it ([`current`]) and resolves it against its config
//! ([`MemoryIdentity::namespace`]) to stamp what an agent learns and to
//! confine what it recalls to its [`Reach`]: its own node and, unless
//! `[memory.agents.<id>] inherit = false`, the nodes above it. A sibling
//! agent's memory is never in reach.
//!
//! The identity is never taken from model arguments.

use std::future::Future;

use tinymemory::{Namespace, Reach, Segment, SegmentKind};

use crate::config::Config;

tokio::task_local! {
    static CURRENT: MemoryIdentity;
}

/// Who is acting on memory: the agent, the agents that spawned it, and its
/// team. Scoping one needs no config; the node it maps to is resolved
/// against the config of whoever reads or writes ([`Self::namespace`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryIdentity {
    /// The agent definition id; `None` outside any agent (RPC, sync jobs).
    pub agent_id: Option<String>,
    /// The agents that spawned this one, outermost first.
    pub lineage: Vec<String>,
    /// The team the agent works in.
    pub team: Option<String>,
}

impl MemoryIdentity {
    /// The root identity: no agent, the shared node.
    #[must_use]
    pub fn root() -> Self {
        Self::default()
    }

    /// A top-level agent.
    #[must_use]
    pub fn agent(agent_id: &str) -> Self {
        Self {
            agent_id: Some(agent_id.to_string()).filter(|id| !id.trim().is_empty()),
            ..Self::default()
        }
    }

    /// `agent_id` as a member of `team`.
    #[must_use]
    pub fn team_member(team: &str, agent_id: &str) -> Self {
        Self {
            team: Some(team.to_string()).filter(|team| !team.trim().is_empty()),
            ..Self::agent(agent_id)
        }
    }

    /// `agent_id` run by this agent: a sub-agent, nested under this agent.
    #[must_use]
    pub fn child(&self, agent_id: &str) -> Self {
        let mut lineage = self.lineage.clone();
        lineage.extend(self.agent_id.clone());
        Self {
            agent_id: Some(agent_id.to_string()).filter(|id| !id.trim().is_empty()),
            lineage,
            team: self.team.clone(),
        }
    }

    /// The node this agent writes to under `config`. Walking from the team's
    /// node (or the root) down the lineage to the agent: an agent the config
    /// pins (`[memory.agents.<id>] namespace`) moves to its pinned node, a
    /// root agent stays where it is, and any other agent nests one level
    /// (`agent:<id>`).
    #[must_use]
    pub fn namespace(&self, config: &Config) -> Namespace {
        let mut node = self.team.as_deref().map_or(Namespace::ROOT, |team| {
            nest(&Namespace::ROOT, SegmentKind::Team, team)
        });
        for agent in self.lineage.iter().chain(self.agent_id.iter()) {
            node = step(config, &node, agent);
        }
        node
    }

    /// Whether this agent also reads the nodes above its own
    /// (`[memory.agents.<id>] inherit`, on by default).
    #[must_use]
    pub fn inherit(&self, config: &Config) -> bool {
        self.agent_id
            .as_deref()
            .and_then(|id| config.memory.agents.get(id))
            .is_none_or(|settings| settings.inherit)
    }

    /// What this agent reads: its node, and its ancestors when it inherits.
    #[must_use]
    pub fn reach(&self, config: &Config) -> Reach {
        let node = self.namespace(config);
        if self.inherit(config) {
            Reach::of(node)
        } else {
            Reach::exact(node)
        }
    }
}

/// One step down from `node` for `agent_id`.
fn step(config: &Config, node: &Namespace, agent_id: &str) -> Namespace {
    let pinned = config
        .memory
        .agents
        .get(agent_id)
        .and_then(|settings| parse_namespace(settings.namespace.as_deref()));
    match pinned {
        Some(pinned) => pinned,
        None if agent_id.trim().is_empty() || is_root_agent(config, agent_id) => node.clone(),
        None => nest(node, SegmentKind::Agent, agent_id),
    }
}

/// The node `agent_id` owns at the top level, optionally as a member of
/// `team`: its pinned `[memory.agents.<id>] namespace`, the team's node (or
/// the root) for a root agent (`[memory] root_agents`, the main chat agent
/// by default), else `team:<team>/agent:<id>` or `agent:<id>`.
#[must_use]
pub fn namespace_for(config: &Config, agent_id: &str, team: Option<&str>) -> Namespace {
    match team {
        Some(team) => MemoryIdentity::team_member(team, agent_id),
        None => MemoryIdentity::agent(agent_id),
    }
    .namespace(config)
}

/// Whether `agent_id` reads and writes the root node.
#[must_use]
pub fn is_root_agent(config: &Config, agent_id: &str) -> bool {
    config
        .memory
        .root_agents
        .iter()
        .any(|root| root == agent_id)
}

/// `namespace` with a sanitized `kind:id` segment appended; past the depth
/// limit the node itself is returned, so a runaway spawn chain shares its
/// deepest node rather than failing.
fn nest(namespace: &Namespace, kind: SegmentKind, id: &str) -> Namespace {
    namespace
        .child(Segment::sanitized(kind, id))
        .unwrap_or_else(|_| namespace.clone())
}

/// A configured namespace; an invalid one is logged and ignored.
fn parse_namespace(raw: Option<&str>) -> Option<Namespace> {
    let raw = raw?.trim();
    match raw.parse::<Namespace>() {
        Ok(namespace) => Some(namespace),
        Err(error) => {
            tracing::warn!(namespace = raw, %error, "[memory:scope] ignoring an invalid configured namespace");
            None
        }
    }
}

/// Runs `fut` as `identity`.
pub async fn within<F: Future>(identity: MemoryIdentity, fut: F) -> F::Output {
    tracing::debug!(
        agent_id = identity.agent_id.as_deref().unwrap_or("-"),
        depth = identity.lineage.len(),
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

/// Runs a turn of `agent_id`: as a sub-agent of the identity already in
/// scope when another agent is running, otherwise as the agent itself. A
/// turn of the agent already in scope keeps its identity.
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
