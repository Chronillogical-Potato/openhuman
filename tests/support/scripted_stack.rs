//! Small pieces of the scripted-upstream harness shared by the agent e2e
//! targets (`agent_harness_e2e`, `in_process/agent_prompt_comprehension_e2e`):
//! the mock backend's completion shapes, the `/auth/me` stub, and the
//! JSON-RPC result unwrap they all use.
//!
//! The scripted router itself (queue, capture, SSE collector, boot stack) stays
//! in each suite: the two differ in how they script and capture requests.
//!
//! Include with `#[path = "support/scripted_stack.rs"] mod scripted_stack;` or
//! declare it at the root of an aggregated target.

#![allow(dead_code)]

use std::sync::{Mutex, MutexGuard};

use axum::http::HeaderMap;
use axum::Json;
use serde_json::{json, Value};

/// Lock `m`, recovering the data when a panicking test poisoned it.
pub fn lock_or_recover<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    match m.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    }
}

/// A plain-text assistant completion for the scripted upstream.
pub fn text_completion(content: &str) -> Value {
    json!({ "content": content })
}

/// One completion carrying several parallel tool calls in `toolCalls`.
pub fn tool_calls_completion(calls: &[(&str, Value)]) -> Value {
    json!({ "content": "", "toolCalls": calls.iter().map(|(name, arguments)| json!({
        "id": format!("call_{name}_{}", arguments.to_string().len()),
        "name": name,
        "arguments": arguments.to_string(),
    })).collect::<Vec<_>>() })
}

/// The `/auth/me` stub the mock backend serves.
pub async fn current_user(_headers: HeaderMap) -> Json<Value> {
    Json(json!({ "success": true, "data": { "_id": "e2e-user-1", "username": "e2e" } }))
}

/// The envelope's `result`; panics with `context` on a JSON-RPC error.
pub fn assert_no_jsonrpc_error<'a>(v: &'a Value, context: &str) -> &'a Value {
    if let Some(err) = v.get("error") {
        panic!("{context}: JSON-RPC error: {err}");
    }
    v.get("result")
        .unwrap_or_else(|| panic!("{context}: missing result: {v}"))
}
