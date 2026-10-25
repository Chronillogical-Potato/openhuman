//! Fail-closed host boundary for the separately loaded TinySecurity module.
//!
//! This slice establishes the loader, typed bus client and callback boundary.
//! Existing OpenHuman security callers remain in place until their migration
//! issues move them explicitly.

pub(super) mod callbacks;
mod client;
mod policy;

#[cfg(test)]
#[path = "security_tests.rs"]
mod tests;

pub use client::{Client, SecurityError};
pub use policy::policy_config;
pub(super) use policy::{module_config, module_config_for_host};

use crate::config::Config;

/// Registry id for the released TinySecurity policy module.
pub const MODULE_ID: &str = "tinysecurity";

/// Load the pinned module before a caller requests its proxy.
///
/// # Errors
/// Returns an error if the component is disabled or its pinned module cannot load.
pub async fn ensure_loaded(config: &Config) -> Result<(), SecurityError> {
    policy::validate(config).map_err(|error| SecurityError::Unavailable(error.into()))?;
    if !config.modules.enabled {
        return Err(SecurityError::Unavailable("modules are disabled".into()));
    }
    super::ops::ensure_loaded(config, MODULE_ID)
        .await
        .map_err(SecurityError::Unavailable)
}

/// Construct a client over the attested module object.
///
/// # Errors
/// Missing, incompatible, and faulted modules are errors and never imply allow.
pub async fn client(config: &Config) -> Result<Client, SecurityError> {
    ensure_loaded(config).await?;
    Client::connect(config)
        .await
        .map_err(SecurityError::Unavailable)
}

/// Replace the running module configuration through the loader control plane.
/// Any reinitialization failure is returned to the caller; policy consumers must
/// keep denying while the configuration is uncertain.
pub async fn reinitialize(config: &Config) -> Result<(), SecurityError> {
    ensure_loaded(config).await?;
    let runtime = super::runtime::runtime()
        .await
        .map_err(|_| SecurityError::Unavailable("module runtime unavailable".into()))?;
    let trusted_host = runtime
        .connection()
        .unique_name()
        .map(|name| name.to_string());
    let config = policy::module_config_for_host(config, trusted_host)
        .map_err(|error| SecurityError::Unavailable(error.into()))?;
    let payload = serde_json::to_value(config)
        .map_err(|_| SecurityError::Unavailable("invalid module configuration".into()))?;
    runtime
        .connection()
        .reinitialize_module(MODULE_ID, payload)
        .await
        .map(|_| ())
        .map_err(|error| {
            tracing::warn!(error = %error, "security module reinitialization failed");
            SecurityError::Unavailable("security module reinitialization failed".into())
        })
}
