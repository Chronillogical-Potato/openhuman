//! The SaaS gateway layer: which context each request runs under.
//!
//! Replaces the single-context layer when the process runs in SaaS mode. For
//! every request it:
//!
//! 1. answers `404` for routes a SaaS core never serves — the OpenAI-compatible
//!    `/v1`, the `/events/*` debug streams, the WebSockets, `/dev/connect` and
//!    the MCP OAuth callback — and for the `/events` chat stream outside a
//!    user's scope;
//! 2. with no `X-OpenHuman-User`, runs it on the operator plane (the bearer
//!    check downstream still applies);
//! 3. with one, checks the service bearer **first** — so an unauthenticated
//!    caller learns nothing about which users exist and cannot open agents —
//!    then the signature, then runs the request under that user's agent.
//!
//! The decision itself lives in `openhuman_core::user_agents::gateway`.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::Request;
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use openhuman_core::core::runtime::CoreContext;
use openhuman_core::user_agents::gateway::{
    resolve_scope, GatewayScope, USER_HEADER, USER_SIG_HEADER,
};

/// Route prefixes a SaaS core never serves.
pub(crate) const CLOSED_IN_SAAS: &[&str] = &[
    "/v1",
    "/events/",
    "/ws/",
    "/socket.io",
    "/dev/connect",
    "/oauth/",
];

pub(crate) fn is_closed_in_saas(path: &str) -> bool {
    CLOSED_IN_SAAS.iter().any(|prefix| {
        path == prefix.trim_end_matches('/')
            || path.starts_with(prefix)
                && (prefix.ends_with('/') || path[prefix.len()..].starts_with('/'))
    })
}

fn refuse(status: u16, message: &str) -> Response {
    let status = StatusCode::from_u16(status).unwrap_or(StatusCode::FORBIDDEN);
    (status, axum::Json(serde_json::json!({ "error": message }))).into_response()
}

fn header_str<'a>(req: &'a Request, name: &str) -> Option<&'a str> {
    req.headers().get(name).and_then(|v| v.to_str().ok())
}

fn bearer(req: &Request) -> Option<&str> {
    header_str(req, header::AUTHORIZATION.as_str())?
        .strip_prefix("Bearer ")
        .map(str::trim)
}

/// The SaaS request layer; `operator` is the runtime's own context.
pub(crate) async fn saas_gateway(operator: Arc<CoreContext>, req: Request, next: Next) -> Response {
    let path = req.uri().path();
    if is_closed_in_saas(path) {
        log::debug!("[rpc:saas] {path} is not served in SaaS mode");
        return refuse(404, "not found");
    }

    let Some(user) = header_str(&req, USER_HEADER).map(str::to_owned) else {
        // The chat event stream is a user's; the operator has none.
        if path == "/events" {
            return refuse(404, "not found");
        }
        return CoreContext::scope(operator, next.run(req)).await;
    };

    let Some(secret) = openhuman_core::core::auth::get_rpc_token() else {
        return refuse(503, "the core is not ready");
    };
    if !bearer(&req).is_some_and(openhuman_core::core::auth::verify_bearer_token) {
        return refuse(401, "unauthorized");
    }
    let signature = header_str(&req, USER_SIG_HEADER).map(str::to_owned);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    match resolve_scope(Some(&user), signature.as_deref(), secret, now) {
        Ok(GatewayScope::User(agent)) => {
            let ctx = Arc::clone(agent.context());
            // Holding the state for the request keeps the agent from being
            // evicted under it.
            let response = CoreContext::scope(ctx, next.run(req)).await;
            drop(agent);
            response
        }
        Ok(GatewayScope::Operator) => CoreContext::scope(operator, next.run(req)).await,
        Err(refusal) => refuse(refusal.status, &refusal.message),
    }
}

#[cfg(test)]
#[path = "saas_gateway_tests.rs"]
mod tests;
