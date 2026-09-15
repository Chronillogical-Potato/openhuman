//! Shared fixtures: an in-process backend stub (axum) that answers
//! `POST /auth/login-token/consume` and `GET /auth/me`, and a fake
//! [`CoreLink`] with the credential store the core would keep.

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use tokio::sync::Mutex as AsyncMutex;

use crate::link::{self, CoreLink};

/// Serialises tests that touch process environment variables.
pub static ENV_LOCK: AsyncMutex<()> = AsyncMutex::const_new(());

/// A JWT (alg none) with `sub`/`userId` = `user-123` and `exp` in 2100.
pub const LIVE_JWT: &str = "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.eyJzdWIiOiJ1c2VyLTEyMyIsInVzZXJJZCI6InVzZXItMTIzIiwiZXhwIjo0MTAyNDQ0ODAwfQ.sig";
/// Same claims, `exp` in 2001.
pub const EXPIRED_JWT: &str = "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.eyJzdWIiOiJ1c2VyLTEyMyIsImV4cCI6OTc4MzA3MjAwfQ.sig";
/// A JWT with an `exp` but no subject claim.
pub const LIVE_JWT_NO_SUB: &str = "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.eyJleHAiOjQxMDI0NDQ4MDB9.sig";
/// Opaque, not a JWT.
pub const OPAQUE_TOKEN: &str = "mock-jwt-token";
pub const LOCAL_TOKEN: &str = "eyJhbGciOiJub25lIn0.eyJzdWIiOiJsb2NhbCJ9.local";

/// One scripted `/auth/me` answer.
#[derive(Debug, Clone)]
pub enum MeAnswer {
    Ok(Value),
    Status(u16),
    /// Sleep this long before answering OK (for timeout tests).
    Slow(u64),
}

#[derive(Default)]
pub struct StubState {
    pub me: Mutex<VecDeque<MeAnswer>>,
    pub me_calls: Mutex<Vec<HeaderMap>>,
    pub consume_calls: Mutex<Vec<Value>>,
    pub consume_jwt: Mutex<Option<String>>,
}

pub struct Backend {
    pub url: String,
    pub state: Arc<StubState>,
}

impl Backend {
    /// The `/auth/me` answers in order; the last one repeats forever.
    pub async fn start(answers: Vec<MeAnswer>) -> Self {
        let state = Arc::new(StubState::default());
        *state.me.lock().unwrap() = answers.into();
        *state.consume_jwt.lock().unwrap() = Some(LIVE_JWT.to_string());
        let app = Router::new()
            .route("/auth/me", get(handle_me))
            .route("/auth/login-token/consume", post(handle_consume))
            .with_state(Arc::clone(&state));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            url: format!("http://{addr}"),
            state,
        }
    }

    pub fn me_calls(&self) -> usize {
        self.state.me_calls.lock().unwrap().len()
    }

    pub fn consume_calls(&self) -> Vec<Value> {
        self.state.consume_calls.lock().unwrap().clone()
    }
}

pub fn me_user() -> Value {
    json!({ "_id": "user-123", "email": "u@example.com", "name": "User" })
}

async fn handle_me(State(state): State<Arc<StubState>>, headers: HeaderMap) -> impl IntoResponse {
    state.me_calls.lock().unwrap().push(headers);
    let answer = {
        let mut queue = state.me.lock().unwrap();
        if queue.len() > 1 {
            queue.pop_front()
        } else {
            queue.front().cloned()
        }
    };
    match answer.unwrap_or(MeAnswer::Ok(me_user())) {
        MeAnswer::Ok(user) => (StatusCode::OK, Json(json!({ "success": true, "data": user }))).into_response(),
        MeAnswer::Status(code) => (
            StatusCode::from_u16(code).unwrap(),
            Json(json!({ "success": false, "message": format!("status {code}") })),
        )
            .into_response(),
        MeAnswer::Slow(ms) => {
            tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
            (StatusCode::OK, Json(json!({ "success": true, "data": me_user() }))).into_response()
        }
    }
}

async fn handle_consume(State(state): State<Arc<StubState>>, Json(body): Json<Value>) -> impl IntoResponse {
    state.consume_calls.lock().unwrap().push(body.clone());
    let token = body.get("token").and_then(Value::as_str).unwrap_or("");
    if token == "expired" {
        return (StatusCode::UNAUTHORIZED, Json(json!({ "success": false, "message": "expired" }))).into_response();
    }
    let jwt = state.consume_jwt.lock().unwrap().clone().unwrap_or_default();
    (StatusCode::OK, Json(json!({ "success": true, "data": { "jwt": jwt } }))).into_response()
}

/// What the fake core holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoredCredential {
    pub kind: String,
    pub token: String,
    pub user_id: Option<String>,
    pub user: Option<Value>,
}

/// A [`CoreLink`] that behaves like the core's `auth.*` credential RPCs,
/// records every call, and reports `api_url`.
#[derive(Default)]
pub struct FakeCore {
    pub api_url: Mutex<String>,
    pub session: Mutex<Option<StoredCredential>>,
    pub api_key: Mutex<Option<String>>,
    pub calls: Mutex<Vec<(String, Value)>>,
    /// When set, every invoke fails with this message.
    pub fail_with: Mutex<Option<String>>,
}

impl FakeCore {
    pub fn new(api_url: &str) -> Arc<Self> {
        let core = Self::default();
        *core.api_url.lock().unwrap() = api_url.to_string();
        Arc::new(core)
    }

    pub fn calls(&self) -> Vec<(String, Value)> {
        self.calls.lock().unwrap().clone()
    }

    pub fn methods(&self) -> Vec<String> {
        self.calls().into_iter().map(|(m, _)| m).collect()
    }

    pub fn session(&self) -> Option<StoredCredential> {
        self.session.lock().unwrap().clone()
    }

    fn state(&self) -> Value {
        if let Some(key) = self.api_key.lock().unwrap().as_ref() {
            let _ = key;
            return json!({ "isAuthenticated": true, "credential": "api-key", "userId": null, "user": null, "profileId": null });
        }
        match self.session.lock().unwrap().as_ref() {
            Some(s) => json!({
                "isAuthenticated": true,
                "credential": s.kind,
                "userId": s.user_id,
                "user": s.user,
                "profileId": "app-session:default",
            }),
            None => json!({ "isAuthenticated": false, "userId": null, "user": null, "profileId": null }),
        }
    }
}

#[async_trait]
impl CoreLink for FakeCore {
    async fn invoke(&self, method: &str, params: Value) -> Result<Value, String> {
        self.calls.lock().unwrap().push((method.to_string(), params.clone()));
        if let Some(message) = self.fail_with.lock().unwrap().clone() {
            return Err(message);
        }
        match method {
            link::CONFIG_RESOLVE_API_URL => Ok(json!({ "api_url": self.api_url.lock().unwrap().clone() })),
            link::AUTH_GET_STATE => Ok(self.state()),
            link::AUTH_GET_SESSION_TOKEN => Ok(json!({
                "result": { "token": self.session.lock().unwrap().as_ref().map(|s| s.token.clone()) },
                "logs": ["session token fetched"],
            })),
            link::AUTH_SET_CREDENTIAL => {
                let token = params.get("token").and_then(Value::as_str).unwrap_or("").to_string();
                let kind = params.get("kind").and_then(Value::as_str).unwrap_or("session").to_string();
                if token.is_empty() {
                    return Err("token is required".to_string());
                }
                if kind == "api-key" {
                    *self.api_key.lock().unwrap() = Some(token);
                } else {
                    *self.session.lock().unwrap() = Some(StoredCredential {
                        kind,
                        token,
                        user_id: params.get("userId").and_then(Value::as_str).map(str::to_string),
                        user: params.get("user").cloned(),
                    });
                }
                Ok(self.state())
            }
            link::AUTH_CLEAR_CREDENTIAL => {
                match params.get("kind").and_then(Value::as_str) {
                    Some("api-key") => *self.api_key.lock().unwrap() = None,
                    Some(_) => *self.session.lock().unwrap() = None,
                    None => {
                        *self.api_key.lock().unwrap() = None;
                        *self.session.lock().unwrap() = None;
                    }
                }
                Ok(json!({ "cleared": true }))
            }
            other => Err(format!("unknown method {other}")),
        }
    }
}
