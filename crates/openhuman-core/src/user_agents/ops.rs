//! Operator-plane operations on user agents.
//!
//! Gateway user ids are taken here and turned into agent ids at once; they
//! are never logged, stored or returned.

use super::host::{self, AgentHost};
use super::types::{
    DeprovisionResult, ProvisionResult, UserAgentId, UserAgentSummary,
};
use crate::core::Outcome;

fn require_host() -> Result<std::sync::Arc<AgentHost>, String> {
    host::host().ok_or_else(|| "user agents exist only in SaaS mode".to_string())
}

/// Create the agent for gateway user `user_id`, if it does not exist yet.
pub fn provision(user_id: &str) -> Result<Outcome<ProvisionResult>, String> {
    provision_on(&*require_host()?, user_id)
}

pub(crate) fn provision_on(
    host: &AgentHost,
    user_id: &str,
) -> Result<Outcome<ProvisionResult>, String> {
    let agent_id = UserAgentId::for_user(user_id)?;
    let created = host.provision(&agent_id)?;
    let log = if created {
        format!("provisioned {agent_id}")
    } else {
        format!("{agent_id} was already provisioned")
    };
    Ok(Outcome::single_log(ProvisionResult { agent_id, created }, log))
}

/// Close agent `agent_id` and archive its state.
pub fn deprovision(agent_id: &str) -> Result<Outcome<DeprovisionResult>, String> {
    deprovision_on(&*require_host()?, agent_id)
}

pub(crate) fn deprovision_on(
    host: &AgentHost,
    agent_id: &str,
) -> Result<Outcome<DeprovisionResult>, String> {
    let agent_id = UserAgentId::parse(agent_id)?;
    let removed = host.deprovision(&agent_id)?;
    let log = if removed {
        format!("archived {agent_id}")
    } else {
        format!("{agent_id} was not provisioned")
    };
    Ok(Outcome::single_log(DeprovisionResult { agent_id, removed }, log))
}

/// Every provisioned agent.
pub fn list() -> Result<Outcome<Vec<UserAgentSummary>>, String> {
    let agents = require_host()?.list()?;
    let log = format!("{} user agent(s)", agents.len());
    Ok(Outcome::single_log(agents, log))
}

/// One agent, or an error when it is not provisioned.
pub fn status(agent_id: &str) -> Result<Outcome<UserAgentSummary>, String> {
    status_on(&*require_host()?, agent_id)
}

pub(crate) fn status_on(
    host: &AgentHost,
    agent_id: &str,
) -> Result<Outcome<UserAgentSummary>, String> {
    let agent_id = UserAgentId::parse(agent_id)?;
    let summary = host
        .summary(&agent_id)?
        .ok_or_else(|| format!("agent {agent_id} is not provisioned"))?;
    Ok(Outcome::single_log(summary, format!("status of {agent_id}")))
}

#[cfg(test)]
#[path = "ops_tests.rs"]
mod tests;
