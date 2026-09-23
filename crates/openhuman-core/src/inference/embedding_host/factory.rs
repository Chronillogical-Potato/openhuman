//! OpenHuman bindings for TinyInference's embedding factory.

use std::sync::Arc;

use crate::config::Config;
use tinyinference_embeddings::factory::{
    create_configured_embedding_model, create_default_embedding_model, CredentialResolver,
    EmbeddingFactorySettings, ManagedModelFactory,
};

use super::cloud::OpenHumanCloudEmbeddingModel;
use super::provider_trait::{EmbeddingProvider, TinyInferenceEmbeddingProvider};

fn settings(
    provider: &str,
    model: &str,
    dimensions: usize,
    ollama_base_url: String,
) -> EmbeddingFactorySettings {
    EmbeddingFactorySettings {
        provider: provider.to_owned(),
        model: model.to_owned(),
        dimensions,
        ollama_base_url,
    }
}

fn managed_factory(config: &Config) -> ManagedModelFactory {
    let config = config.clone();
    Arc::new(move |model, dimensions| {
        Ok(Box::new(OpenHumanCloudEmbeddingModel::from_config(
            &config, model, dimensions,
        )))
    })
}

fn credential_resolver(config: &Config) -> CredentialResolver {
    let config = config.clone();
    Arc::new(move |provider| super::rpc::resolve_api_key(&config, provider))
}

/// Creates a provider from explicit settings and host credentials.
pub fn create_embedding_provider_with_credentials(
    provider: &str,
    model: &str,
    dimensions: usize,
    api_key: &str,
    custom_endpoint: Option<&str>,
) -> anyhow::Result<Box<dyn EmbeddingProvider>> {
    let provider = custom_endpoint
        .filter(|_| provider == "custom")
        .map(|endpoint| format!("custom:{endpoint}"))
        .unwrap_or_else(|| provider.to_owned());
    let api_key = api_key.to_owned();
    let credentials: CredentialResolver = Arc::new(move |_| api_key.clone());
    let managed: ManagedModelFactory = Arc::new(|model, dimensions| {
        Ok(Box::new(OpenHumanCloudEmbeddingModel::new_default_scope(
            model, dimensions,
        )))
    });
    let model = create_configured_embedding_model(
        &settings(
            &provider,
            model,
            dimensions,
            tinyinference_local::ollama::ollama_base_url(),
        ),
        &credentials,
        &managed,
    )?;
    Ok(Box::new(TinyInferenceEmbeddingProvider::from_boxed(model)))
}

/// Creates a provider using OpenHuman's credential and endpoint configuration.
pub fn create_embedding_provider_with_config(
    config: &Config,
    provider: &str,
    model: &str,
    dimensions: usize,
    api_key: &str,
    custom_endpoint: Option<&str>,
) -> anyhow::Result<Box<dyn EmbeddingProvider>> {
    let provider = custom_endpoint
        .filter(|_| provider == "custom")
        .map(|endpoint| format!("custom:{endpoint}"))
        .unwrap_or_else(|| provider.to_owned());
    let selected = settings(
        &provider,
        model,
        dimensions,
        tinyinference_local::ollama::ollama_base_url_from_override(
            config.local_ai.base_url.as_deref(),
        ),
    );
    let api_key = api_key.to_owned();
    let credentials: CredentialResolver = Arc::new(move |_| api_key.clone());
    let model =
        create_configured_embedding_model(&selected, &credentials, &managed_factory(config))?;
    Ok(Box::new(TinyInferenceEmbeddingProvider::from_boxed(model)))
}

/// Returns the configured provider, falling back to managed embeddings.
pub fn default_embedding_provider_with_config(config: &Config) -> Arc<dyn EmbeddingProvider> {
    let selected = settings(
        &config.memory.embedding_provider,
        &config.memory.embedding_model,
        config.memory.embedding_dimensions,
        tinyinference_local::ollama::ollama_base_url_from_override(
            config.local_ai.base_url.as_deref(),
        ),
    );
    let model = create_default_embedding_model(
        &selected,
        &credential_resolver(config),
        &managed_factory(config),
    )
    .unwrap_or_else(|error| {
        log::warn!("[embeddings::factory] managed fallback failed to build: {error}");
        Box::new(tinyinference_embeddings::NoopEmbeddingModel)
    });
    Arc::new(TinyInferenceEmbeddingProvider::from_boxed(model))
}

#[cfg(test)]
#[path = "factory_tests.rs"]
mod tests;
