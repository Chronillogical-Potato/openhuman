//! Socket domain — persistent Socket.IO client connection to the backend.
//!
//! Provides the `SocketManager` for WebSocket-based communication with
//! automatic reconnection, MCP tool dispatch, webhook routing, and channel
//! inbound message handling.

mod event_handlers;
pub mod models;
mod ops;
mod port;
mod schemas;
pub(crate) mod token_provider;

pub use port::{
    global_socket_manager, global_socket_port, set_global_socket_manager, set_global_socket_port,
    SocketIdentityRebindGuard, SocketPort, TokenProvider,
};
pub use event_handlers::dispatch_socket_event;
pub use schemas::{
    all_controller_schemas as all_socket_controller_schemas,
    all_registered_controllers as all_socket_registered_controllers,
};

pub(crate) use schemas::all_controller_entries;

#[cfg(test)]
pub(crate) use schemas::socket_parameter_contracts;
