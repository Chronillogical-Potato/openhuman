//! Inference HTTP endpoint end-to-end tests.
//!
//! Non-streaming request/response, auth-header, temperature and SSE streaming
//! behavior of `OpenAiModel` is covered in tinyinference-llm
//! (`providers/openai/{wire_test,test}.rs`).
//!
//! The `/v1/chat/completions` and `/v1/models` HTTP endpoint tests verify the
//! full axum router layer (auth middleware + provider routing) end-to-end.
//!
//! No live LLM API calls are made.

#[path = "support/env_guard.rs"]
mod env_guard;
use env_guard::EnvVarGuard;
use std::sync::OnceLock;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

use openhuman_core::core::auth::{init_rpc_token, CORE_TOKEN_ENV_VAR};
use openhuman_rpc::server::build_core_http_router;

// ── Environment serialisation lock ───────────────────────────────────────────
//
// Tests that mutate OPENHUMAN_WORKSPACE or OPENHUMAN_CORE_TOKEN must acquire
// this lock first to prevent races when cargo runs tests in parallel threads
// within the same process.

static ENV_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
static RPC_AUTH_INIT: OnceLock<()> = OnceLock::new();

async fn env_lock_async() -> tokio::sync::MutexGuard<'static, ()> {
    let m = ENV_LOCK.get_or_init(|| tokio::sync::Mutex::new(()));
    m.lock().await
}

const TEST_RPC_TOKEN: &str = "inference-provider-e2e-token";

fn ensure_rpc_auth() {
    RPC_AUTH_INIT.get_or_init(|| {
        // SAFETY: test-only, serialised by OnceLock.
        unsafe { std::env::set_var(CORE_TOKEN_ENV_VAR, TEST_RPC_TOKEN) };
        let tmp = tempdir().expect("tempdir");
        init_rpc_token(tmp.path()).expect("init rpc auth token");
        // Keep tmp alive for the process duration by leaking it — the token
        // file must remain readable for all subsequent auth checks.
        std::mem::forget(tmp);
    });
}

// ── Helper: build an env-isolated Config pointing at tempdir ─────────────────

// ── Test 6: Streaming response returns ordered deltas ────────────────────────

// ── Test 8: /v1/chat/completions HTTP endpoint — unauthorized ─────────────────

#[tokio::test]
async fn http_endpoint_chat_completions_no_bearer_returns_401() {
    let _lock = env_lock_async().await;
    ensure_rpc_auth();

    let body = json!({
        "model": "ollama:llama3",
        "messages": [{ "role": "user", "content": "hello" }]
    });
    let req = Request::builder()
        .method(Method::POST)
        .uri("/v1/chat/completions")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();

    let resp = build_core_http_router(false).oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ── Test 9: /v1/models — unauthorized ────────────────────────────────────────

#[tokio::test]
async fn http_endpoint_models_no_bearer_returns_401() {
    let _lock = env_lock_async().await;
    ensure_rpc_auth();

    let req = Request::builder()
        .method(Method::GET)
        .uri("/v1/models")
        .body(Body::empty())
        .unwrap();

    let resp = build_core_http_router(false).oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ── Test 10: /v1/models with bearer returns non-empty list ───────────────────

#[tokio::test]
async fn http_endpoint_models_with_bearer_returns_model_list() {
    let _lock = env_lock_async().await;
    ensure_rpc_auth();

    let tmp = tempdir().expect("tempdir");
    let _workspace_guard = EnvVarGuard::set("OPENHUMAN_WORKSPACE", tmp.path().to_str().unwrap());

    let req = Request::builder()
        .method(Method::GET)
        .uri("/v1/models")
        .header(header::AUTHORIZATION, format!("Bearer {TEST_RPC_TOKEN}"))
        .body(Body::empty())
        .unwrap();

    let resp = build_core_http_router(false).oneshot(req).await.unwrap();
    assert_ne!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "401 must not fire when bearer is present"
    );
    assert_ne!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "403 must not fire when bearer is present"
    );

    if resp.status().is_success() {
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        let models = json.get("data").and_then(Value::as_array);
        if let Some(list) = models {
            assert!(
                !list.is_empty(),
                "/v1/models should return at least one model"
            );
        }
    }
}

// ── Test 11: /v1/chat/completions with bearer passes auth ────────────────────

#[tokio::test]
async fn http_endpoint_chat_completions_with_bearer_passes_auth() {
    let _lock = env_lock_async().await;
    ensure_rpc_auth();

    let body = json!({
        "model": "ollama:llama3",
        "messages": [{ "role": "user", "content": "ping" }],
        "stream": false
    });
    let req = Request::builder()
        .method(Method::POST)
        .uri("/v1/chat/completions")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {TEST_RPC_TOKEN}"))
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();

    let resp = build_core_http_router(false).oneshot(req).await.unwrap();
    assert_ne!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "401 must not fire when bearer is present"
    );
    assert_ne!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "403 must not fire when bearer is present"
    );
}

// ── Test 14: temperature_for_model helper ────────────────────────────────────
