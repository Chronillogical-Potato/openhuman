//! Host-supplied tools a session is built with, beside the ones config names.
//!
//! A session host built through [`OpenHumanSessionHost::builder`] takes a tool
//! belt directly — an embedder hands it `Box<dyn Tool>` objects and they are
//! the belt. A session built *from config*
//! ([`from_config_with_definition`](super::super::OpenHumanSessionHost::from_config_with_definition))
//! cannot: it is reconstructed on every turn from `Config` and an
//! `AgentDefinition`, both of which are data, so nothing carrying a `dyn Tool`
//! survives between turns. That is why an `openhuman_embed::Agent` has only
//! ever reached a host's own tools over MCP.
//!
//! [`HostTools`] is the seam that closes it, and it is a **factory rather than
//! a belt** for exactly the reason above: `Box<dyn Tool>` is not `Clone` and
//! `Agent` is, so a stored belt could not survive the per-turn rebuild. The
//! closure is invoked once per turn, which also means the belt it returns may
//! differ from turn to turn — a host whose tools are bound to something
//! shorter-lived than the agent (one episode, one room, one assignment) can
//! express that here instead of registering a second agent for it.
//!
//! The prompt's tool catalogue is rendered from the same belt in the same
//! build, so a changing belt and its description stay consistent **on a turn
//! that composes its prompt**. A resumed session reuses its persisted system
//! messages, so a belt that moves under one is described by the prompt it had
//! when the thread opened; a host that varies its belt should run such turns
//! on a session of their own.

use std::collections::HashSet;
use std::sync::Arc;

use tinytools::Tool;

use crate::agent::tool_policy::ToolPolicy;

/// One turn's worth of host-supplied belt.
///
/// `visible` is the provider-visible allow-list to union into the session's
/// own; leaving it empty makes the tools reachable but unadvertised, which is
/// rarely what a host wants. `policy`, when set, becomes the session's gate --
/// see [`with_policy`](Self::with_policy), which is not what "host tools"
/// might suggest.
#[derive(Default)]
pub struct HostTurnTools {
    /// The tools themselves, placed **ahead of** the config-derived belt so a
    /// host tool wins a collision on its name.
    pub tools: Vec<Box<dyn Tool>>,
    /// Names to add to the provider-visible allow-list.
    pub visible: HashSet<String>,
    /// The session's admission gate, if the host sets one.
    pub policy: Option<Arc<dyn ToolPolicy>>,
}

impl HostTurnTools {
    /// A belt with every tool advertised, which is the common case.
    #[must_use]
    pub fn advertised(tools: Vec<Box<dyn Tool>>) -> Self {
        let visible = tools.iter().map(|tool| tool.name().to_string()).collect();
        Self {
            tools,
            visible,
            policy: None,
        }
    }

    /// Sets the gate for the whole session.
    ///
    /// # This replaces; it does not wrap
    ///
    /// The policy set here becomes the session's tool policy outright -- it is
    /// not consulted first and then deferred to a config-derived one, because
    /// there is no composition step to defer through.
    ///
    /// That is what the episode case wants: a gate saying *admit my belt, and
    /// ask me about everything else* is a statement about the whole session,
    /// not only about the tools the host supplied. But it means **a host that
    /// gates only its own names denies every other tool on the belt**. If the
    /// session should keep an existing policy for calls the host does not own,
    /// the host composes the two and passes the result here.
    #[must_use]
    pub fn with_policy(mut self, policy: Arc<dyn ToolPolicy>) -> Self {
        self.policy = Some(policy);
        self
    }

    /// Whether this contributes nothing, so a caller can skip the union.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty() && self.visible.is_empty() && self.policy.is_none()
    }
}

impl std::fmt::Debug for HostTurnTools {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostTurnTools")
            .field(
                "tools",
                &self
                    .tools
                    .iter()
                    .map(|tool| tool.name())
                    .collect::<Vec<_>>(),
            )
            .field("visible", &self.visible)
            .field("policy", &self.policy.is_some())
            .finish()
    }
}

/// What the turn being built is, as far as a host's tool factory needs to know.
///
/// A belt that varies has to vary on *something*. Without this the factory is
/// called with no argument and has to infer its own occasion from state it
/// closed over, which works only while the agent serves one conversation at a
/// time -- the moment it serves two, a belt bound to "the current episode" is
/// a race rather than a decision.
///
/// Non-exhaustive: this describes an occasion, and occasions gain detail.
/// Construct it with [`TurnContext::new`] and read it through the accessors so
/// a later field cannot break a host that matched on it.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct TurnContext<'a> {
    agent_id: &'a str,
    session_id: Option<&'a str>,
}

impl<'a> TurnContext<'a> {
    /// The turn's occasion: which agent, and which conversation if one was named.
    #[must_use]
    pub fn new(agent_id: &'a str, session_id: Option<&'a str>) -> Self {
        Self {
            agent_id,
            session_id,
        }
    }

    /// The agent definition this turn runs as.
    #[must_use]
    pub fn agent_id(&self) -> &'a str {
        self.agent_id
    }

    /// The conversation this turn runs in, as the caller named it.
    ///
    /// `None` when no session was named -- a one-shot turn, or a path that
    /// mints an id only after the session is built. A host keying its belt on
    /// this should decide what an unnamed turn gets rather than assume it
    /// cannot happen.
    #[must_use]
    pub fn session_id(&self) -> Option<&'a str> {
        self.session_id
    }
}

/// Builds one turn's host belt.
///
/// Invoked once per **session build**, which on the paths a host reaches --
/// `agent_chat` builds a session for every turn -- means once per turn. The
/// distinction matters for anything that composes sessions differently: the
/// guarantee is per build, not per turn, and a build that is reused serves the
/// belt it was built with.
///
/// See [`TurnContext`] for what the factory is told about the occasion.
pub type HostTools = Arc<dyn for<'a> Fn(TurnContext<'a>) -> HostTurnTools + Send + Sync>;
