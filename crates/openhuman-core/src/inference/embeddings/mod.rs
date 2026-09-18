//! Embedding providers for the OpenHuman memory system.
//!
//! Converts text into numerical vectors for semantic search. Providers:
//!
//! - **Managed** (default): Routes through the OpenHuman backend's
//!   `POST /openai/v1/embeddings` (Voyage-backed). The recommended path —
//!   works on a fresh install without requiring a local Ollama daemon.
//! - **Voyage**: Direct Voyage AI API with the user's own key.
//! - **OpenAI**: Cloud-based embeddings via the OpenAI API.
//! - **Cohere**: Cohere embed API with the user's own key.
//! - **Ollama**: Local Ollama server. Opt-in for offline-only setups.
//! - **Custom**: Any OpenAI-compatible endpoint.
//! - **Noop**: A fallback provider for keyword-only search.

#[path = "cloud_adapter.rs"]
pub mod cloud;
mod factory;
pub mod noop;
mod provider_trait;
mod rpc;
mod schemas;

pub use cloud::{
    OpenHumanCloudEmbedding, DEFAULT_CLOUD_EMBEDDING_DIMENSIONS, DEFAULT_CLOUD_EMBEDDING_MODEL,
};
pub use factory::{
    create_embedding_provider, create_embedding_provider_with_config,
    create_embedding_provider_with_credentials, default_embedding_provider,
    default_embedding_provider_with_config, default_local_embedding_provider,
};
pub use noop::NoopEmbedding;
pub use provider_trait::{
    format_embedding_signature, EmbeddingProvider, TinyAgentsEmbeddingProvider,
};
pub use rpc::provider_from_config;
// Reached through this re-export by `modules::memory_host`, which serves the
// seam over the bus. `memory::host_impls` served the same seam in-process and
// reached it the same way, until the in-process engine left the test build too
// (openhuman#6161) and took that file with it. `embeddings::rpc` itself names
// the function through `super::rpc`, not through here, so this gate does not
// narrow it.
#[cfg(any(test, feature = "modules"))]
pub(crate) use rpc::resolve_api_key;
pub use schemas::{
    all_controller_schemas as all_embeddings_controller_schemas,
    all_registered_controllers as all_embeddings_registered_controllers,
};
use tinyinference_core::embeddings::{DEFAULT_OLLAMA_DIMENSIONS, DEFAULT_OLLAMA_MODEL};

/// The **intended** embedding selection — `(provider, model, dimensions)`.
///
/// # Why this is the host's and not the engine's
///
/// Which embedder the operator meant is selection policy over the host's own
/// config — the same class of decision as the preference lanes and the event
/// heuristics before it. The engine kept an identical helper for its internal
/// pipelines; this host used to reach through the crate for it, which was an
/// engine link taken on for a ten-line precedence rule (#5560). Ported
/// verbatim: a configured local model wins over the `[memory]` section, and a
/// blank local value falls back to the Ollama default rather than shipping
/// whitespace to a daemon that will 404 it.
///
/// Note: this is the *intended* setting. It does not check whether the Ollama
/// daemon is actually running.
pub fn effective_embedding_settings(
    memory: &crate::config::schema::MemoryConfig,
    local_embedding_model: Option<&str>,
) -> (String, String, usize) {
    if let Some(raw) = local_embedding_model {
        // Trim once and reuse — the emptiness check and the final model
        // string must agree, otherwise a value like "  bge-m3  " would pass
        // through to Ollama with surrounding whitespace and 404.
        let trimmed = raw.trim();
        let model = if trimmed.is_empty() {
            DEFAULT_OLLAMA_MODEL.to_string()
        } else {
            trimmed.to_string()
        };
        return ("ollama".to_string(), model, DEFAULT_OLLAMA_DIMENSIONS);
    }
    (
        memory.embedding_provider.clone(),
        memory.embedding_model.clone(),
        memory.embedding_dimensions,
    )
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
