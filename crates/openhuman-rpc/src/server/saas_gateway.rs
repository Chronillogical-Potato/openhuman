//! The SaaS gateway layer: which context each request runs under.
//!
//! Replaces the single-context layer when the process runs in SaaS mode. For
//! every request it:
//!
//! 1. answers `404` for routes a SaaS core never serves — the OpenAI-compatible
//!    `/v1`, every `/events` stream, the WebSockets, `/dev/connect` and the MCP
//!    OAuth callback;
//! 2. with no `X-OpenHuman-User`, runs it on the operator plane (the bearer
//!    check downstream still applies);
//! 3. with one, checks the service bearer **first** — so an unauthenticated
//!    caller learns nothing about which users exist and cannot open agents —
//!    then the signature, then runs the request under that user's agent.
//!
//! The decision itself lives in `crate::core_host::user_agents::gateway`.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::core_host::core::runtime::CoreContext;
use crate::core_host::user_agents::gateway::{
    resolve_scope, GatewayScope, USER_HEADER, USER_SIG_HEADER,
};
use axum::extract::Request;
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

/// Route prefixes a SaaS core never serves.
pub(crate) const CLOSED_IN_SAAS: &[&str] = &[
    "/v1",
    "/events",
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

    // Absent means the operator plane. Present but unreadable, or present
    // twice, is refused: falling back to the operator would skip the user
    // signature check.
    let mut user_headers = req.headers().get_all(USER_HEADER).iter();
    let Some(first) = user_headers.next() else {
        return CoreContext::scope(operator, next.run(req)).await;
    };
    if user_headers.next().is_some() {
        log::debug!("[rpc:saas] refusing a request with more than one {USER_HEADER}");
        return refuse(400, "more than one user header");
    }
    let Ok(user) = first.to_str().map(str::to_owned) else {
        log::debug!("[rpc:saas] refusing an unreadable {USER_HEADER}");
        return refuse(400, "unreadable user header");
    };

    let Some(secret) = crate::core_host::core::auth::get_rpc_token() else {
        return refuse(503, "the core is not ready");
    };
    if !bearer(&req).is_some_and(crate::core_host::core::auth::verify_bearer_token) {
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
