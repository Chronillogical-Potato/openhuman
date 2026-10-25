//! Typed, attested calls to the TinySecurity policy interface.

use tinysecurity_bus::{CheckRequest, CheckResponse, PolicyInfo};

use super::MODULE_ID;
use crate::config::Config;

/// Sanitized failure from the security module boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecurityError {
    /// The module is absent, disabled, faulted, or failed to answer.
    Unavailable(String),
}

impl std::fmt::Display for SecurityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(reason) => write!(f, "security module unavailable: {reason}"),
        }
    }
}

impl std::error::Error for SecurityError {}

/// A client whose object path comes from the compiled registry record.
#[derive(Clone)]
pub struct Client {
    object: tinybus::module::ModuleObjectClient,
}

impl Client {
    pub(super) async fn connect(config: &Config) -> Result<Self, String> {
        let client = super::super::ops::client(config.clone());
        let object = client
            .object_client(MODULE_ID, tinysecurity_bus::names::OBJECT_PATH)
            .await
            .map_err(|_| "pinned security module proxy unavailable".to_owned())?;
        Ok(Self { object })
    }

    /// Read effective settings and advertised capabilities from the module.
    pub async fn policy_info(&self) -> Result<PolicyInfo, SecurityError> {
        self.object
            .call_confidential(tinysecurity_bus::names::methods::POLICY_INFO, ())
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }

    /// Ask the module for a policy decision. A missing or malformed response
    /// remains an error for the caller to deny.
    pub async fn check(&self, request: &CheckRequest) -> Result<CheckResponse, SecurityError> {
        self.object
            .call_confidential(tinysecurity_bus::names::methods::CHECK, (request,))
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }
}
