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
mod provider_trait;
mod rpc;
mod schemas;

pub use cloud::{
    OpenHumanCloudEmbeddingModel, DEFAULT_CLOUD_EMBEDDING_DIMENSIONS, DEFAULT_CLOUD_EMBEDDING_MODEL,
};
pub use factory::{
    create_embedding_provider_with_config, create_embedding_provider_with_credentials,
    default_embedding_provider_with_config,
};
pub use provider_trait::{
    format_embedding_signature, EmbeddingProvider, TinyInferenceEmbeddingProvider,
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
#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
