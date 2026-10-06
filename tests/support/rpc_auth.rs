//! The RPC bearer for suites that serve the core router in-process.
//!
//! `core::auth::RPC_TOKEN` is a process-global `OnceLock` and `init_rpc_token`
//! is first-writer-wins, so a suite that hard-codes its own token gets a 401
//! whenever another suite sharing the process initialised first. Suites ask
//! for whatever token is actually active instead of choosing one: with no
//! `OPENHUMAN_CORE_TOKEN` in play, `init_rpc_token` mints a fresh token into a
//! per-process directory, so this adds no env mutation of its own.
//!
//! Include with `#[path = "support/rpc_auth.rs"] mod rpc_auth;` or declare it at
//! the root of an aggregated target.

#![allow(dead_code)]

use std::sync::OnceLock;

static TOKEN: OnceLock<String> = OnceLock::new();

/// Initialise the core's RPC token subsystem once and return the live token.
pub fn rpc_token() -> &'static str {
    TOKEN.get_or_init(|| {
        let dir =
            std::env::temp_dir().join(format!("openhuman-e2e-rpc-auth-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create the rpc token dir");
        openhuman_core::core::auth::init_rpc_token(&dir).expect("init the core rpc auth token");
        openhuman_core::core::auth::get_rpc_token()
            .expect("init_rpc_token leaves a token in place")
            .to_string()
    })
}

/// Make sure the token exists before a router is built or served.
pub fn ensure_rpc_auth() {
    let _ = rpc_token();
}
