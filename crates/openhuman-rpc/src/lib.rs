//! OpenHuman's JSON-RPC protocol, on both sides of the wire.
//!
//! The core owns what a controller *is*: its schema, the
//! `Outcome` it returns, and in-process
//! dispatch (`openhuman::core::invoke::invoke_method`). This crate owns
//! how that is exposed over JSON-RPC 2.0, and sits above the core:
//!
//! - [`RpcRequest`], [`RpcSuccess`], [`RpcFailure`] and [`RpcError`] are the
//!   envelopes the server reads and writes; [`request_body`] and
//!   [`decode_response`] are the client half, and [`unwrap_rpc`] reaches a
//!   handler's value through its log envelope.
//! - [`is_origin_allowed_with_extra`] and [`ALLOWED_ORIGINS_ENV`] are the
//!   browser-origin allowlist for the HTTP API.
//! - Behind `http-client`: [`post_json_rpc`], [`bearer_header`],
//!   [`redact_url_for_log`] and [`HttpRpcResponse`].
//! - Behind `server`: [`server`], the core's HTTP router, Socket.IO transport
//!   and listener, plus the `run_server*` entry points hosts call; and
//!   [`http_host`], the static-directory file server whose `http_host.*`
//!   controllers the server registers with the core.
//! - Behind `session-store` (on with `server`): [`session_store`], the
//!   on-disk session store the app, the CLI and the TUI install. The core has
//!   no storage layout of its own, but falls back to workspace files when a
//!   host installs no provider; this is not a guarantee that the core is
//!   persistence-free.

#[cfg(feature = "http-client")]
mod client;
mod envelope;
#[cfg(feature = "server")]
pub mod http_host;
mod origin;
#[cfg(feature = "server")]
pub mod server;
#[cfg(feature = "session-store")]
pub mod session_store;

#[cfg(feature = "http-client")]
pub use client::{bearer_header, post_json_rpc, redact_url_for_log, HttpRpcResponse};
pub use envelope::{
    decode_response, request_body, RpcError, RpcFailure, RpcRequest, RpcSuccess, JSONRPC_VERSION,
    SERVER_ERROR_CODE,
};
pub use crate::core_host::core::unwrap_rpc;
pub use origin::{is_origin_allowed_with_extra, ALLOWED_ORIGINS_ENV};
