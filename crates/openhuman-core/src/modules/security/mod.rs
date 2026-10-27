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
mod device_pairing_store;
pub(crate) mod gap;
pub(super) mod host_callbacks;
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

/// Resolve the admitted TinySecurity client, subject, and short-lived User
/// master handle for a direct paired-device controller action.
pub(crate) async fn device_pairing_master(
    config: &Config,
) -> Result<(Client, crate::security::approval::native::SecretScope, String, u64), String> {
    let client = client(config)
        .await
        .map_err(|error| error.to_string())?;
    let scope = host_callbacks::secret_scope()
        .await
        .ok_or_else(|| "security secret scope unavailable".to_owned())?;
    let registration_id = host_callbacks::secret_registration_id()
        .await
        .ok_or_else(|| "security master registration unavailable".to_owned())?;
    let master = resolve_user_master(&client, &scope, &registration_id)
        .await
        .map_err(|error| error.to_string())?;
    let generation = client
        .policy_info()
        .await
        .map_err(|error| error.to_string())?
        .generation;
    Ok((client, scope, master, generation))
}

/// Mint the event id that binds a direct user controller call to one trusted
/// pairing operation.
pub(crate) fn device_pairing_controller_event(
    scope: &crate::security::approval::native::SecretScope,
    master_handle: String,
    generation: u64,
    channel_id: String,
    pairing_token: String,
) -> Result<String, String> {
    let expires_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "system clock unavailable".to_owned())?
        .as_millis()
        .saturating_add(30_000)
        .min(u128::from(u64::MAX)) as u64;
    callbacks::issue_pairing_event(tinysecurity_bus::DevicePairingContextResponse2 {
        scope_id: scope.scope_id().to_owned(),
        user_binding: scope.scope_id().to_owned(),
        secret_master_handle: Some(master_handle),
        legacy_import_handle: None,
        channel_id,
        pairing_token: Some(pairing_token),
        connection_nonce: uuid::Uuid::new_v4().simple().to_string(),
        generation,
        source: tinysecurity_bus::DevicePairingSource2::AuthenticatedUserController,
        expires_at_ms,
    })
}

/// Mint a direct authenticated user event for module-side revoke/unlock work.
pub(crate) fn device_pairing_user_event(
    scope: &crate::security::approval::native::SecretScope,
    master_handle: Option<String>,
    generation: u64,
) -> Result<String, String> {
    let expires_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "system clock unavailable".to_owned())?
        .as_millis()
        .saturating_add(30_000)
        .min(u128::from(u64::MAX)) as u64;
    let scope_id = scope.scope_id().to_owned();
    callbacks::issue_pairing_event(tinysecurity_bus::DevicePairingContextResponse2 {
        scope_id: scope_id.clone(),
        user_binding: scope_id,
        secret_master_handle: master_handle,
        legacy_import_handle: None,
        channel_id: String::new(),
        pairing_token: None,
        connection_nonce: String::new(),
        generation,
        source: tinysecurity_bus::DevicePairingSource2::AuthenticatedUserController,
        expires_at_ms,
    })
}

/// Mint a direct-user event for a channel lookup/revoke that does not need a
/// secret master alias, such as recovering an imported pairing id.
pub(crate) async fn device_pairing_recover_event(
    config: &Config,
) -> Result<String, String> {
    let client = client(config)
        .await
        .map_err(|error| error.to_string())?;
    let scope = host_callbacks::secret_scope()
        .await
        .ok_or_else(|| "security secret scope unavailable".to_owned())?;
    let generation = client
        .policy_info()
        .await
        .map_err(|error| error.to_string())?
        .generation;
    device_pairing_user_event(&scope, None, generation)
}

/// Mint a one-shot direct-user import event and bind its callback to the
/// current scoped legacy device store and fixed TinySecurity source ids.
pub(crate) async fn device_pairing_legacy_import_event(
    config: &Config,
) -> Result<String, String> {
    let client = client(config)
        .await
        .map_err(|error| error.to_string())?;
    let scope = host_callbacks::secret_scope()
        .await
        .ok_or_else(|| "security secret scope unavailable".to_owned())?;
    let registration_id = host_callbacks::secret_registration_id()
        .await
        .ok_or_else(|| "security master registration unavailable".to_owned())?;
    let master_handle = resolve_user_master(&client, &scope, &registration_id)
        .await
        .map_err(|error| error.to_string())?;
    let generation = client
        .policy_info()
        .await
        .map_err(|error| error.to_string())?
        .generation;
    let expires_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "system clock unavailable".to_owned())?
        .as_millis()
        .saturating_add(30_000)
        .min(u128::from(u64::MAX)) as u64;
    let scope_id = scope.scope_id().to_owned();
    let handle = uuid::Uuid::new_v4().simple().to_string();
    let backend_id = policy::pairing_legacy_backend_id(&registration_id);
    callbacks::register_legacy_import_grant(
        handle.clone(),
        config.clone(),
        scope_id.clone(),
        scope_id.clone(),
        generation,
        backend_id,
        registration_id,
        expires_at_ms,
    )?;
    callbacks::issue_pairing_event(tinysecurity_bus::DevicePairingContextResponse2 {
        scope_id: scope_id.clone(),
        user_binding: scope_id,
        secret_master_handle: Some(master_handle),
        legacy_import_handle: Some(handle),
        channel_id: String::new(),
        pairing_token: None,
        connection_nonce: String::new(),
        generation,
        source: tinysecurity_bus::DevicePairingSource2::AuthenticatedUserController,
        expires_at_ms,
    })
}

/// Mint an event capability only for a frame delivered by the authenticated
/// hosted socket adapter. The socket nonce and short expiry are supplied by
/// that adapter path, never from tunnel RPC parameters.
pub(crate) async fn device_pairing_socket_event(
    config: &Config,
    channel_id: &str,
    pairing_token: &str,
    connection_nonce: &str,
    expires_at_ms: u64,
) -> Result<(String, String), String> {
    if channel_id.is_empty()
        || channel_id.len() > 128
        || pairing_token.is_empty()
        || pairing_token.len() > 4096
        || connection_nonce.is_empty()
        || connection_nonce.len() > 128
    {
        return Err("invalid host tunnel event".to_owned());
    }
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "system clock unavailable".to_owned())?
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;
    if expires_at_ms <= now_ms || expires_at_ms.saturating_sub(now_ms) > 30_000 {
        return Err("expired host tunnel event".to_owned());
    }
    let client = client(config)
        .await
        .map_err(|error| error.to_string())?;
    let scope = host_callbacks::secret_scope()
        .await
        .ok_or_else(|| "security secret scope unavailable".to_owned())?;
    let generation = client
        .policy_info()
        .await
        .map_err(|error| error.to_string())?
        .generation;
    let scope_id = scope.scope_id().to_owned();
    let event_id = callbacks::issue_pairing_event(tinysecurity_bus::DevicePairingContextResponse2 {
        scope_id: scope_id.clone(),
        user_binding: scope_id.clone(),
        secret_master_handle: None,
        legacy_import_handle: None,
        channel_id: channel_id.to_owned(),
        pairing_token: Some(pairing_token.to_owned()),
        connection_nonce: connection_nonce.to_owned(),
        generation,
        source: tinysecurity_bus::DevicePairingSource2::RegisteredSocket,
        expires_at_ms,
    })?;
    Ok((event_id, scope_id))
}

/// Mint the server-observed close capability for one socket nonce.
pub(crate) async fn device_pairing_socket_closed_event(
    config: &Config,
    channel_id: &str,
    connection_nonce: &str,
    expires_at_ms: u64,
) -> Result<(String, String), String> {
    if channel_id.is_empty() || channel_id.len() > 128 || connection_nonce.is_empty() {
        return Err("invalid host socket close event".to_owned());
    }
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "system clock unavailable".to_owned())?
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;
    if expires_at_ms <= now_ms || expires_at_ms.saturating_sub(now_ms) > 30_000 {
        return Err("expired host socket close event".to_owned());
    }
    let client = client(config)
        .await
        .map_err(|error| error.to_string())?;
    let scope = host_callbacks::secret_scope()
        .await
        .ok_or_else(|| "security secret scope unavailable".to_owned())?;
    let generation = client
        .policy_info()
        .await
        .map_err(|error| error.to_string())?
        .generation;
    let scope_id = scope.scope_id().to_owned();
    let event_id = callbacks::issue_pairing_event(tinysecurity_bus::DevicePairingContextResponse2 {
        scope_id: scope_id.clone(),
        user_binding: scope_id.clone(),
        secret_master_handle: None,
        legacy_import_handle: None,
        channel_id: channel_id.to_owned(),
        pairing_token: None,
        connection_nonce: connection_nonce.to_owned(),
        generation,
        source: tinysecurity_bus::DevicePairingSource2::SocketClosed,
        expires_at_ms,
    })?;
    Ok((event_id, scope_id))
}

pub(crate) async fn resolve_user_master_with_existing_key(
    client: &Client,
    scope: &crate::security::approval::native::SecretScope,
    registration_id: &str,
    encoded_key: &str,
) -> Result<String, SecurityError> {
    let backend = super::ops::security_secret_backend().await.ok_or_else(|| {
        SecurityError::Unavailable("TinySecurity secret backend unavailable".into())
    })?;
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
