//! OpenHuman policy, RPC, and speech bindings for `tinyinference-local`.
//!
//! Runtime discovery, model management, downloads, and inference execution
//! live in TinyInference. This module retains only product-owned access
//! policy, controller wiring, and voice integration.

#[cfg(test)]
pub(crate) static INFERENCE_TEST_MUTEX: once_cell::sync::Lazy<std::sync::Mutex<()>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(()));

#[cfg(test)]
pub(crate) fn inference_test_guard() -> std::sync::MutexGuard<'static, ()> {
    INFERENCE_TEST_MUTEX
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

mod core;
pub mod ops;
mod schemas;

// `pub(crate)` so the shared `apply_no_window` helper can be reused from the
// agent shell runtime (`agent::host_runtime`) — single source of truth for the
// Windows `CREATE_NO_WINDOW` flag (#3727/#3728).
pub mod service;

pub use core::*;
pub use ops as rpc;
pub use ops::*;
pub use schemas::{
    all_controller_schemas as all_local_inference_controller_schemas,
    all_registered_controllers as all_local_inference_registered_controllers,
};
pub use service::LocalAiService;
