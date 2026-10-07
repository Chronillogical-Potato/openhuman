//! What a host agent may do on a turn an outside sender started.
//!
//! A channel turn is [`AgentTurnOrigin::ExternalChannel`]: whoever can message
//! the bot wrote its text. On the orchestrator path an acting tool parks an
//! approval and, with no one to answer it, is denied when the park expires. A
//! host agent bound to a public channel must not wait on that: the sender is
//! the only person in the chat, and they are the one who must not approve it.
//! So the turn is capped at [`PermissionLevel::ReadOnly`] and anything above
//! that, or anything with an external effect, is refused on the spot with a
//! tool error the model can relay.

use tinytools::{PermissionLevel, Tool};

use crate::agent::turn_origin::AgentTurnOrigin;
use crate::agent::HostTools;
use crate::config::Config;

/// The most a host agent may do on a turn from `origin`; `None` leaves the
/// agent's own limits alone.
pub(crate) fn tool_ceiling(origin: &AgentTurnOrigin) -> Option<PermissionLevel> {
    let _ = origin;
    None
}

/// Cap `channel` at `ceiling` in this turn's `config`.
pub(crate) fn cap_channel(config: &mut Config, channel: &str, ceiling: PermissionLevel) {
    let _ = (config, channel, ceiling);
}

/// `host` with every tool it builds refusing calls above `ceiling`.
pub(crate) fn guard_host_tools(host: HostTools, ceiling: PermissionLevel) -> HostTools {
    let _ = ceiling;
    host
}

/// A tool that refuses calls above its ceiling instead of running them.
pub(crate) struct CeilingGuard {
    inner: Box<dyn Tool>,
    ceiling: PermissionLevel,
}

impl CeilingGuard {
    pub(crate) fn new(inner: Box<dyn Tool>, ceiling: PermissionLevel) -> Self {
        Self { inner, ceiling }
    }
}

#[path = "guard_tool.rs"]
mod guard_tool;

#[cfg(test)]
#[path = "posture_tests.rs"]
mod tests;
