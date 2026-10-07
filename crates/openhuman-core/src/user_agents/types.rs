//! Serde types for user agents: one agent per SaaS user.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Longest gateway user id accepted, in bytes.
pub const MAX_USER_ID_LEN: usize = 256;

/// The agent that serves one SaaS user.
///
/// Derived from the gateway's user id by hashing, never stored alongside it:
/// the id is stable for a user, fits the agent-id charset
/// (`[a-z0-9_-]{1,64}`), and puts no user identifier into paths, logs or
/// memory namespaces.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct UserAgentId(String);

const PREFIX: &str = "u-";
const HASH_HEX_LEN: usize = 32;

impl UserAgentId {
    /// The agent for gateway user `user_id`.
    pub fn for_user(user_id: &str) -> Result<Self, String> {
        if user_id.is_empty() {
            return Err("user id is empty".to_string());
        }
        if user_id.len() > MAX_USER_ID_LEN {
            return Err(format!("user id is longer than {MAX_USER_ID_LEN} bytes"));
        }
        if user_id.chars().any(char::is_control) {
            return Err("user id contains control characters".to_string());
        }
        let digest = Sha256::digest(user_id.as_bytes());
        let hex = hex::encode(digest);
        Ok(Self(format!("{PREFIX}{}", &hex[..HASH_HEX_LEN])))
    }

    /// Parse an agent id as [`Self::for_user`] produces it.
    pub fn parse(raw: &str) -> Result<Self, String> {
        let hash = raw
            .strip_prefix(PREFIX)
            .ok_or_else(|| format!("`{raw}` is not a user agent id"))?;
        if hash.len() != HASH_HEX_LEN
            || !hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(format!("`{raw}` is not a user agent id"));
        }
        Ok(Self(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for UserAgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for UserAgentId {
    type Error = String;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        Self::parse(&raw)
    }
}

impl From<UserAgentId> for String {
    fn from(id: UserAgentId) -> Self {
        id.0
    }
}

/// Written beside a provisioned agent's state, so the operator plane can list
/// agents without opening them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserAgentMeta {
    pub agent_id: UserAgentId,
    /// Unix seconds.
    pub created_at: u64,
    /// Layout version, for future migrations.
    pub layout_version: u32,
}

/// The current [`UserAgentMeta::layout_version`].
pub const LAYOUT_VERSION: u32 = 1;

/// What [`provision`](super::ops::provision) did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProvisionResult {
    pub agent_id: UserAgentId,
    /// `false` when the agent already existed.
    pub created: bool,
}

/// What [`deprovision`](super::ops::deprovision) did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeprovisionResult {
    pub agent_id: UserAgentId,
    /// `false` when there was no such agent.
    pub removed: bool,
}

/// One provisioned agent, as the operator plane sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UserAgentSummary {
    pub agent_id: UserAgentId,
    pub created_at: u64,
    /// Whether it is loaded in this process right now.
    pub open: bool,
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
