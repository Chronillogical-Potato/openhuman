//! Core internals for the crates layered directly above embed
//! (`openhuman-tinyhumans`, `openhuman-rpc`), and nothing else.
//!
//! Those layers implement the backend transport, the hosted controllers and
//! the JSON-RPC server, which reach deep into the core by nature. Routing
//! them through this module keeps the dependency chain strict (they need no
//! `openhuman-core` dependency of their own) without inventing a public API
//! for every server internal. It is an explicit list of modules, not a glob:
//! a new reach-in is added here on purpose. Hosts (app, CLI, TUI) use the
//! curated facade at the crate root instead.

pub use openhuman_core::{
    agent, backend, channels, config, core, desktop, inference, integrations, mcp, platform,
    security, tools, util, voice, web3, web_chat,
};

pub use openhuman_core::run_core_from_args;
