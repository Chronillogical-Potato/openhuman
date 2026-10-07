//! Channel turns answered by a host-registered agent.

use crate::config::Config;

pub(crate) mod posture;

/// The agent `channel` is bound to in `config.agent.channel_agents`, if any.
pub(crate) fn bound_agent_id<'a>(config: &'a Config, channel: &str) -> Option<&'a str> {
    config
        .agent
        .channel_agents
        .get(channel)
        .map(|id| id.trim())
        .filter(|id| !id.is_empty())
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "dispatch_tests.rs"]
mod dispatch_tests;
