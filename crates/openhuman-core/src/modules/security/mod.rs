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

pub(crate) use client::SecurityProxy;
pub use client::{Client, SecurityError};
pub use policy::policy_config;
pub(super) use policy::{module_config, module_config_for_host, module_config_for_secret_scope};
pub(crate) use secret_cipher::Output as SecretCipherOutput;

pub(crate) async fn resolve_user_master(
    client: &Client,
    scope: &crate::security::approval::native::SecretScope,
    registration_id: &str,
) -> Result<String, SecurityError> {
    secret_cipher::resolve_user_master(client, scope, registration_id).await
}

pub(crate) async fn resolve_user_master_with_existing_key(
    client: &Client,
    scope: &crate::security::approval::native::SecretScope,
    registration_id: &str,
    encoded_key: &str,
) -> Result<String, SecurityError> {
    let backend = super::ops::security_secret_backend()
        .await
        .ok_or_else(|| SecurityError::Unavailable("TinySecurity secret backend unavailable".into()))?;
    secret_cipher::resolve_user_master_with_existing_key(
        client,
        scope,
        registration_id,
        &backend,
        encoded_key,
    )
    .await
}

pub(crate) async fn run_secret_cipher(
    client: &Client,
    scope: &crate::security::approval::native::SecretScope,
    consumer: &cipher_consumer::Consumer,
    registration_id: &str,
    master_handle: &str,
    operation: tinysecurity_bus::SecretCipherKind2,
    input: &[u8],
) -> Result<SecretCipherOutput, SecurityError> {
    secret_cipher::run(
        client,
        scope,
        consumer,
        registration_id,
        master_handle,
        operation,
        input,
    )
    .await
}

/// Serve the cipher output and master storage callbacks on the exact admitted
/// TinySecurity host binding.
pub(super) async fn serve_secret_callbacks(
    binding: &tinybus::module::HostCallbackBinding,
    consumer: std::sync::Arc<cipher_consumer::Consumer>,
    backend: std::sync::Arc<secret_backend::SecretBackend>,
) -> tinybus::Result<()> {
    callbacks::serve_on_binding(binding).await?;
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
) -> Result<SecurityProxy, String> {
    ensure_loaded(config)
        .await
        .map_err(|error| error.to_string())?;
    let binding = super::ops::security_callback_binding()
        .await
        .ok_or_else(|| "security callback binding unavailable".to_owned())?;
    let record = super::registry::find(MODULE_ID)
        .ok_or_else(|| "security module registry record missing".to_owned())?;
    let proxy = binding
        .proxy(
            record.bus_name,
            record.object_path,
            tinysecurity_bus::names::INTERFACE,
        )
        .map_err(|_| "security module proxy unavailable".to_owned())?
        .with_timeout(timeout);
    Ok(SecurityProxy::callback(proxy))
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
    let payload = super::ops::security_module_config(config)
        .await
        .map_err(SecurityError::Unavailable)?;
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
