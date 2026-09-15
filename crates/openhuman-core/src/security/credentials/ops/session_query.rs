//! Read-only session state lookups.

use serde_json::json;

use crate::api::config::effective_backend_api_url;
use crate::api::jwt::get_session_token;
use crate::api::rest::BackendOAuthClient;
use crate::config::Config;
use crate::rpc::RpcOutcome;
use crate::security::credentials::session_support::build_session_state;

pub async fn auth_get_state(
    config: &Config,
) -> Result<RpcOutcome<super::super::responses::AuthStateResponse>, String> {
    let state = build_session_state(config)?;
    Ok(RpcOutcome::single_log(state, "session state fetched"))
}

pub async fn auth_get_session_token_json(
    config: &Config,
) -> Result<RpcOutcome<serde_json::Value>, String> {
    let token = get_session_token(config)?;
    Ok(RpcOutcome::single_log(
        json!({ "token": token }),
        "session token fetched",
    ))
}

pub async fn auth_get_me(config: &Config) -> Result<RpcOutcome<serde_json::Value>, String> {
    let api_url = effective_backend_api_url(&config.api_url);
    let token = get_session_token(config)?.ok_or_else(|| "session JWT required".to_string())?;
    let client = BackendOAuthClient::new(&api_url).map_err(|e| e.to_string())?;
    let user = client
        .fetch_current_user(&token)
        .await
        // `flatten_authed_error` maps the typed `BackendApiError::Unauthorized`
        // onto the `SESSION_EXPIRED:` sentinel and falls through to `{e:#}` for
        // everything else, so both properties this call site needs are kept:
        //
        // * Non-401s still render the full anyhow context chain, so the
        //   underlying reqwest transport error (timeout / connection reset /
        //   TLS / DNS) reaches `observability::is_transient_message_failure`.
        //   Bare `e.to_string()` renders only the top context layer
        //   ("GET /auth/me") and collapsed every transient transport failure
        //   into Sentry TAURI-RUST-10.
        // * A 401 is recognised by `jsonrpc::is_session_expired_error`, which
        //   skips the Sentry report AND publishes `DomainEvent::SessionExpired`
        //   so `SessionExpiredSubscriber` clears the dead JWT.
        //
        // Until #5232 routed `fetch_current_user` through `authed_json`, a 401
        // here surfaced as `"GET /auth/me failed (401 Unauthorized): …"`, which
        // `is_session_expired_error` matched on its HTTP-verb prefix. The typed
        // error renders as `"backend rejected session token on GET /auth/me"`,
        // which matches neither classifier — so on 0.63.9 every lapsed session
        // reported to Sentry as a code defect (TAURI-RUST-RYD) and, because the
        // stale token was never cleared, re-fired the same 401 on the next
        // revalidation: the forced sign-out loop in #5307.
        .map_err(crate::api::flatten_authed_error)?;

    Ok(RpcOutcome::single_log(user, "current user fetched"))
}
