//! OpenHuman inference integration domain.
//!
//! TinyInference owns reusable model/provider behavior. This module owns the
//! product-facing configuration, policy, process lifecycle, and RPC seams:
//! - `local/`    — Ollama / LM Studio / Piper runtime management
//!                 (was `local_ai/`)
//! - `provider/` — native chat models, cloud/local routing, auth and errors
//!                 (was `providers/`)
//! - `voice/`    — transcription (STT) and TTS inference implementations
//!                 (moved from `voice/`; audio I/O and the voice RPC surface
//!                 stay in `crate::voice`)
//! - `http/`     — OpenAI-compatible `/v1/chat/completions` endpoint
//! - `embeddings/` — embedding provider selection and RPC
//! - `tokenjuice/` — host adapter for the `tinyjuice` token-compression module
//!
//! The RPC surface is `inference.*`; old `local_ai_*` RPC names are resolved
//! by the legacy alias layer for backwards compatibility.

/// `true` when the crate was compiled with the `inference` feature (the
/// default), i.e. the `cpal` audio-device stack is linked. Lets tests and
/// callers distinguish a slim/headless build from the desktop build without
/// naming gated symbols. When `false`, `cpal` is dropped from the dependency
/// graph (verify with `cargo tree -i cpal`) and the microphone-permission probe
/// reports `Unknown`.
pub const INFERENCE_COMPILED_IN: bool = cfg!(feature = "inference");

pub mod auth_error_registry;
pub mod embeddings;
pub mod http;
pub mod local;
pub mod model_context;
pub mod openai_oauth;
pub mod ops;
pub mod paths;
pub mod provider;
mod schemas;
pub mod tokenjuice;
pub mod types;
pub mod voice;

pub use ops as rpc;
pub use schemas::{
    all_controller_schemas as all_inference_controller_schemas,
    all_registered_controllers as all_inference_registered_controllers, INFERENCE_AGENT_CHAT,
};

// Re-export the types that external callers (voice, agent, etc.) import from inference
pub use local::all_local_inference_controller_schemas;
pub use local::all_local_inference_registered_controllers;
pub use model_context::context_window_for_model;
pub use types::{
    LocalAiAssetStatus, LocalAiAssetsStatus, LocalAiDownloadProgressItem, LocalAiDownloadsProgress,
    LocalAiEmbeddingResult, LocalAiSpeechResult, LocalAiStatus, LocalAiTtsResult,
};

impl tinyinference::local::models::LocalModelConfig for crate::config::Config {
    fn local_provider_name(&self) -> &str {
        &self.local_ai.provider
    }
    fn local_chat_model_id(&self) -> &str {
        &self.local_ai.chat_model_id
    }
    fn local_legacy_model_id(&self) -> &str {
        &self.local_ai.model_id
    }
    fn local_vision_model_id(&self) -> &str {
        &self.local_ai.vision_model_id
    }
    fn local_embedding_model_id(&self) -> &str {
        &self.local_ai.embedding_model_id
    }
    fn local_stt_model_id(&self) -> &str {
        &self.local_ai.stt_model_id
    }
    fn local_tts_voice_id(&self) -> &str {
        &self.local_ai.tts_voice_id
    }
    fn local_quantization(&self) -> &str {
        &self.local_ai.quantization
    }
}

// Test helpers (re-exported for sibling test files that use inference_test_guard)
#[cfg(test)]
pub(crate) fn inference_test_guard() -> std::sync::MutexGuard<'static, ()> {
    local::inference_test_guard()
}
