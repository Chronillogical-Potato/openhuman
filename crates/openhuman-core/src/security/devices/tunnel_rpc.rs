//! Host-owned dispatch for plaintext tunnel RPC requests.
//!
//! Framing and authenticated decryption belong to TinySecurity. This module
//! keeps only the host's controller invocation and response semantics.

use crate::core::runtime::DispatchCtx;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub(super) struct TunnelRpcPayload {
    pub(super) method: String,
    #[serde(default)]
    pub(super) params: Value,
}

pub(super) fn parse_tunnel_rpc_payload(payload: Value) -> Result<TunnelRpcPayload, String> {
    serde_json::from_value(payload).map_err(|error| format!("invalid tunnel RPC payload: {error}"))
}

pub(super) async fn dispatch_tunnel_rpc(
    ctx: &DispatchCtx,
    request: TunnelRpcPayload,
) -> Result<Value, String> {
    crate::core::invoke::invoke_method(
        ctx.clone(),
        crate::core::invoke::default_state(),
        &request.method,
        request.params,
    )
    .await
}

#[cfg(test)]
#[path = "tunnel_rpc_tests.rs"]
mod tests;
