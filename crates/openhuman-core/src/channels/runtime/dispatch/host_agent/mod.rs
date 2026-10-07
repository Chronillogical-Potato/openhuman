//! Channel turns answered by a host-registered agent.

use crate::config::Config;

/// The agent `channel` is bound to in `config.agent.channel_agents`, if any.
pub(crate) fn bound_agent_id<'a>(config: &'a Config, channel: &str) -> Option<&'a str> {
    let _ = (config, channel);
    None
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
