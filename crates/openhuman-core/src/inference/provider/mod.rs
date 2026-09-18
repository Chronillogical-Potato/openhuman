//! Native chat-model construction plus cloud/local inference policy and DTOs.
//!
//! This module was previously `providers/` (in the pre-consolidation,
//! single-crate layout). It now lives under `inference/provider/` so all
//! inference concerns (local runtime, cloud providers, HTTP endpoint) share
//! a single domain root.

pub mod config_rejection;
pub mod error_classify;
pub mod error_code;
pub mod factory;
pub(crate) mod openai_codex;
/// Crate-native managed OpenHuman backend as a host `ChatModel` (issue #4727).
pub mod openhuman_backend_model;
pub mod ops;
pub mod schemas;
pub mod types;

#[allow(unused_imports)]
pub use types::{
    ChatRequest, ChatResponse, ProviderDelta, ToolCall, UsageInfo, AGENT_TURN_MAX_OUTPUT_TOKENS,
};

pub use config_rejection::{
    is_openai_compatible_unknown_model_message, is_provider_config_rejection_message,
    NO_MODEL_CONFIGURED_ANCHOR,
};
pub use error_code::{
    backend_error_code_skips_sentry, body_flags_malformed, extract_backend_error_code,
    extract_backend_error_code_token, is_backend_client_guard_leak,
    is_backend_malformed_bad_request, is_managed_backend_envelope, managed_error_skips_sentry,
    BackendErrorCode,
};

#[cfg(feature = "flows")]
pub(crate) use factory::is_raw_passthrough_model;
pub use factory::{
    create_chat_model, create_chat_model_from_string, create_chat_model_from_string_with_model_id,
    create_chat_model_with_model_id, probe_inference_readiness, provider_for_role,
    role_for_model_tier, BYOK_INCOMPLETE_SENTINEL,
};
pub use openhuman_backend_model::{OpenHumanBackendModel, PROVIDER_LABEL};
pub use ops::*;
