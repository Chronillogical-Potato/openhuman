//! Core credential callback passed through the hosted socket port.
//!
//! Keeping credential lookup at this seam lets the host refresh credentials
//! before each SDK-owned connection attempt.
//!
//! ## Why a callback instead of `String`?
//!
//! The connection transport receives the latest session token before each
//! attempt rather than retaining an expired credential across reconnects.

use std::sync::Arc;
pub(super) use super::TokenProvider;

/// Callable that returns the current session token on demand.
///
/// `Ok(token)` — a non-empty token was available; use it for the next attempt.
/// `Err(reason)` — no token is stored (user logged out, profile corrupt); the
///   caller should surface `reason` and exit the reconnect loop.
///
/// The provider is asynchronous because session lookup may use the configured
/// credential store. TinyHumans bridges it to the SDK callback via the shared
/// storage blocking runtime.

/// Build a static provider that always returns the same token value.
///
/// Used by `SocketManager::connect(url, token)` so the public API does not
/// change: existing callers that already have a token in hand (e.g. the CLI,
/// integration tests) can still pass it as a `&str` without touching the
/// provider layer. On each reconnect the loop will re-call the provider — in
/// this case it returns the same cloned string — which is equivalent to the
/// previous behaviour.
///
/// For **live** session-token refresh, build a provider via
/// `token_provider_from_config` (used by `handle_connect_with_session`).
pub(super) fn static_token_provider(token: String) -> TokenProvider {
    Arc::new(move || {
        let token = token.clone();
        Box::pin(async move {
            if token.trim().is_empty() {
                Err("empty session token — authenticate first".to_string())
            } else {
                Ok(token)
            }
        })
    })
}

/// Build a provider that reads the latest app-session token from the profile
/// store on every call. The channel relay requires a user session.
///
/// This is the **live-refresh** path used by `handle_connect_with_session`:
/// when the loop retries after a disconnect it will see any token that was
/// refreshed or re-stored since the previous attempt.
pub(crate) fn token_provider_from_config(config: Arc<crate::config::Config>) -> TokenProvider {
    Arc::new(move || {
        let config = Arc::clone(&config);
        Box::pin(async move {
            let token = crate::credentials::session_state::get_session_token(&config)
                .await
                .map_err(|e| format!("failed to read session token: {e}"))?
                .ok_or_else(|| "no session token stored — user must log in first".to_string())?;
            if crate::credentials::session_state::is_local_session_token(&token) {
                return Err("offline local session cannot connect to hosted socket".to_string());
            }
            Ok(token)
        })
    })
}

#[cfg(test)]
#[path = "token_provider_tests.rs"]
mod tests;
