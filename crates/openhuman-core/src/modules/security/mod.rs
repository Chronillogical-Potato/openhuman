//! Fail-closed host boundary for the separately loaded TinySecurity module.
//!
//! This slice establishes the loader, typed bus client and callback boundary.
//! Existing OpenHuman security callers remain in place until their migration
//! issues move them explicitly.

pub(crate) mod audit;
pub(super) mod callbacks;
pub(super) mod cipher_consumer;
mod client;
pub(crate) mod command;
pub(crate) mod gap;
mod policy;
pub(crate) mod policy_translation;
pub(crate) mod sandbox;
pub(super) mod secret_backend;
pub(super) mod secret_cipher;
pub(crate) mod sync_seam;
pub(crate) mod text;

#[cfg(test)]
#[path = "security_tests.rs"]
mod tests;

pub use client::{Client, SecurityError};
pub use policy::policy_config;
pub(super) use policy::{
    module_config, module_config_for_host, module_config_for_secret_scope,
};

/// Serve the cipher output and master storage callbacks on the exact admitted
/// TinySecurity host binding.
pub(super) async fn serve_secret_callbacks(
    binding: &tinybus::module::HostCallbackBinding,
    consumer: std::sync::Arc<cipher_consumer::Consumer>,
    backend: std::sync::Arc<secret_backend::SecretBackend>,
) -> tinybus::Result<()> {
    consumer.serve(binding).await?;
    backend.serve(binding).await
}

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
        .map_err(SecurityError::Unavailable)?;
    text::arm(config);
    Ok(())
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

/// Construct the admitted TinySecurity object used by approval lifecycle calls.
pub(crate) async fn approval_object(
    config: &Config,
    timeout: std::time::Duration,
) -> Result<tinybus::module::ModuleObjectClient, String> {
    ensure_loaded(config)
        .await
        .map_err(|error| error.to_string())?;
    super::ops::client(config.clone())
        .with_timeout(timeout)
        .object_client(MODULE_ID, tinysecurity_bus::names::OBJECT_PATH)
        .await
        .map_err(|error| error.to_string())
}

/// Validate a host path using an immutable TinySecurity path scope.
#[cfg(feature = "security-module")]
pub(crate) async fn validate_host_path(
    policy: tinysecurity_bus::PathPolicy,
    path: &str,
    parent: bool,
) -> Result<tinysecurity_bus::PathValidationResult, String> {
    gap::validate_host_path(policy, path, parent).await
}

#[cfg(not(feature = "security-module"))]
pub(crate) async fn validate_host_path(
    _policy: tinysecurity_bus::PathPolicy,
    _path: &str,
    _parent: bool,
) -> Result<tinysecurity_bus::PathValidationResult, String> {
    Err("TinySecurity is not built into this host".into())
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
    let module_name = super::registry::find(MODULE_ID)
        .map(|record| record.source.package)
        .ok_or_else(|| {
            SecurityError::Unavailable("security module registry record missing".into())
        })?;
    runtime
        .connection()
        .reinitialize_module(module_name, payload)
        .await
        .map(|_| ())
        .map_err(|_| SecurityError::Unavailable("security module reinitialization failed".into()))
}
