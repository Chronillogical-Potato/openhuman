//! A user agent's backend credential.
//!
//! The gateway hands the core each user's TinyHumans credential — a session
//! JWT or an API key — through the operator plane. It is stored where every
//! backend caller already looks: the auth-profile store beside the agent's
//! `config_path` (`<root>/agents/<id>/`). Work running under that agent's
//! context loads the agent's config, so `resolve_backend_credential` finds
//! that user's credential and no other.
//!
//! Unlike `auth.set_credential`, this changes nothing process-wide: no
//! `active_user.toml`, no rebinding of the default context, no global identity
//! or Sentry user, no scheduler gate. The core still never validates the
//! credential.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::security::credentials::session_support::{
    has_backend_credential, SESSION_EXPIRES_AT_META,
};
use crate::security::credentials::{
    api_key, AuthService, APP_SESSION_PROVIDER, DEFAULT_AUTH_PROFILE_NAME,
};

/// The kind of credential the gateway installs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserCredentialKind {
    /// A TinyHumans session JWT.
    Session,
    /// A TinyHumans API key.
    ApiKey,
}

/// Store `token` as agent `config`'s credential of `kind`, replacing any
/// credential of the other kind.
///
/// `expires_at` (RFC 3339) lets the core reject an expired session locally
/// instead of sending a doomed request.
pub fn store(
    config: &Config,
    kind: UserCredentialKind,
    token: &str,
    expires_at: Option<&str>,
) -> Result<(), String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("credential is blank".to_string());
    }
    match kind {
        UserCredentialKind::ApiKey => {
            api_key::store_api_key(config, token).map_err(|e| e.to_string())?;
        }
        UserCredentialKind::Session => {
            let mut metadata = HashMap::new();
            if let Some(exp) = expires_at {
                let exp = chrono::DateTime::parse_from_rfc3339(exp)
                    .map_err(|e| format!("expires_at is not RFC 3339: {e}"))?;
                metadata.insert(SESSION_EXPIRES_AT_META.to_string(), exp.to_rfc3339());
            }
            AuthService::from_config(config)
                .store_provider_token(
                    APP_SESSION_PROVIDER,
                    DEFAULT_AUTH_PROFILE_NAME,
                    token,
                    metadata,
                    true,
                )
                .map_err(|e| e.to_string())?;
        }
    }
    // Then drop the other kind, which would otherwise keep winning (an API
    // key is resolved before a session). The new credential is written first,
    // so a failure here never leaves the agent with none; it is reported so
    // the gateway can retry.
    let replaced = match kind {
        UserCredentialKind::Session => api_key::clear_api_key(config).map_err(|e| e.to_string()),
        UserCredentialKind::ApiKey => AuthService::from_config(config)
            .remove_profile(APP_SESSION_PROVIDER, DEFAULT_AUTH_PROFILE_NAME)
            .map_err(|e| e.to_string()),
    };
    if let Err(error) = replaced {
        log::warn!("[user_agents][credentials] stored {kind:?} but could not remove the other kind: {error}");
        return Err(format!(
            "stored the new credential but could not remove the previous one: {error}"
        ));
    }
    log::debug!(
        "[user_agents][credentials] stored {kind:?} credential in {}",
        config
            .config_path
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    );
    Ok(())
}

/// Remove every credential agent `config` holds. Returns whether there was one.
pub fn clear(config: &Config) -> Result<bool, String> {
    let had_key = api_key::clear_api_key(config).map_err(|e| e.to_string())?;
    let had_session = AuthService::from_config(config)
        .remove_profile(APP_SESSION_PROVIDER, DEFAULT_AUTH_PROFILE_NAME)
        .map_err(|e| e.to_string())?;
    Ok(had_key || had_session)
}

/// Whether agent `config` holds a credential.
pub fn has(config: &Config) -> bool {
    has_backend_credential(config)
}

#[cfg(test)]
#[path = "credentials_tests.rs"]
mod tests;
