//! Factory functions for creating embedding providers.

use std::path::PathBuf;
use std::sync::Arc;

use super::cloud::{
    OpenHumanCloudEmbedding, DEFAULT_CLOUD_EMBEDDING_DIMENSIONS, DEFAULT_CLOUD_EMBEDDING_MODEL,
};
use super::provider_trait::{EmbeddingProvider, TinyAgentsEmbeddingProvider};
use crate::config::Config;
use tinyinference::embeddings::{
    model_supports_dimensions, CohereEmbeddingModel, NoopEmbeddingModel, OllamaEmbeddingModel,
    OpenAiEmbeddingModel, VoyageEmbeddingModel,
};

/// Build the provider for an OpenAI-compatible custom endpoint.
///
/// `dims == 0` is the TinyInference dimension-discovery mode (issue #4056):
/// send no `dimensions` parameter, accept the returned vector length, and let
/// the caller adopt it.
fn custom_openai_provider(
    base_url: &str,
    api_key: &str,
    model: &str,
    dims: usize,
) -> anyhow::Result<Box<dyn EmbeddingProvider>> {
    let base_url = validate_custom_endpoint(base_url, !api_key.is_empty())?;
    Ok(TinyAgentsEmbeddingProvider::boxed(openai_model(
        &base_url, api_key, model, dims, false,
    )))
}

fn openai_model(
    base_url: &str,
    api_key: &str,
    model: &str,
    dims: usize,
    required_key: bool,
) -> OpenAiEmbeddingModel {
    OpenAiEmbeddingModel::new(api_key)
        .with_base_url(base_url)
        .with_model(model)
        .with_dimensions(dims)
        .with_send_dimensions(model_supports_dimensions(model))
        .with_required_api_key(required_key)
}

/// Validate a custom endpoint before a provider can send credentials to it.
fn validate_custom_endpoint(endpoint: &str, has_credentials: bool) -> anyhow::Result<String> {
    let endpoint = endpoint.trim().trim_end_matches('/');
    if endpoint.is_empty() {
        anyhow::bail!("custom embedding provider endpoint must not be empty");
    }

    let parsed = reqwest::Url::parse(endpoint)
        .map_err(|_| anyhow::anyhow!("custom embedding provider endpoint is invalid"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        anyhow::bail!("custom embedding provider endpoint must use HTTP or HTTPS");
    }

    if has_credentials {
        let loopback = parsed
            .host()
            .map(|host| match host {
                url::Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
                url::Host::Ipv4(address) => address.is_loopback(),
                url::Host::Ipv6(address) => address.is_loopback(),
            })
            .unwrap_or(false);
        if parsed.scheme() != "https" && !(parsed.scheme() == "http" && loopback) {
            anyhow::bail!(
                "credentialed custom embedding provider endpoints must use HTTPS or loopback HTTP"
            );
        }
    }

    Ok(endpoint.to_owned())
}

/// Creates an embedding provider based on the specified name and configuration.
///
/// Supported provider names:
/// - `"managed"` / `"cloud"` → OpenHuman backend (Voyage-backed) — default
/// - `"voyage"` → direct Voyage AI API (user's own key)
/// - `"openai"` → OpenAI API (user's own key)
/// - `"cohere"` → Cohere API (user's own key)
/// - `"ollama"` → local Ollama server (opt-in for offline-only installs)
/// - `"custom:<url>"` → OpenAI-compatible endpoint
/// - `"none"` → no-op (keyword-only search, no embeddings)
///
/// Returns an error for unrecognised provider names so configuration
/// mistakes surface immediately rather than silently degrading to
/// keyword-only search.
pub fn create_embedding_provider(
    provider: &str,
    model: &str,
    dims: usize,
) -> anyhow::Result<Box<dyn EmbeddingProvider>> {
    match provider {
        "cloud" | "managed" => Ok(Box::new(OpenHumanCloudEmbedding::new(
            None, None, true, model, dims,
        ))),
        "voyage" => Ok(TinyAgentsEmbeddingProvider::boxed(
            VoyageEmbeddingModel::with_options(
                "",
                model,
                dims,
                tinyinference::embeddings::VOYAGE_API_BASE,
            ),
        )),
        "ollama" => {
            let base_url = tinyinference::local::ollama::ollama_base_url();
            Ok(TinyAgentsEmbeddingProvider::boxed(
                OllamaEmbeddingModel::try_new(&base_url, model, dims)?,
            ))
        }
        "openai" => Ok(TinyAgentsEmbeddingProvider::boxed(openai_model(
            "https://api.openai.com",
            "",
            model,
            dims,
            true,
        ))),
        "cohere" => Ok(TinyAgentsEmbeddingProvider::boxed(
            CohereEmbeddingModel::new("")
                .with_model(model)
                .with_dimensions(dims),
        )),
        name if name.starts_with("custom:") => {
            let base_url = name.strip_prefix("custom:").unwrap_or("");
            custom_openai_provider(base_url, "", model, dims)
        }
        "none" => Ok(TinyAgentsEmbeddingProvider::boxed(NoopEmbeddingModel)),
        unknown => Err(anyhow::anyhow!(
            "unknown embedding provider: \"{unknown}\". \
             Supported: \"managed\", \"voyage\", \"openai\", \"cohere\", \
             \"ollama\", \"custom:<url>\", \"none\""
        )),
    }
}

/// Creates an embedding provider with explicit API key and endpoint.
///
/// Used by the RPC layer when credentials are loaded from the credential
/// store.
pub fn create_embedding_provider_with_credentials(
    provider: &str,
    model: &str,
    dims: usize,
    api_key: &str,
    custom_endpoint: Option<&str>,
) -> anyhow::Result<Box<dyn EmbeddingProvider>> {
    match provider {
        "cloud" | "managed" => Ok(Box::new(OpenHumanCloudEmbedding::new(
            None, None, true, model, dims,
        ))),
        "voyage" => Ok(TinyAgentsEmbeddingProvider::boxed(
            VoyageEmbeddingModel::with_options(
                api_key,
                model,
                dims,
                tinyinference::embeddings::VOYAGE_API_BASE,
            ),
        )),
        "ollama" => {
            let base_url = tinyinference::local::ollama::ollama_base_url();
            Ok(TinyAgentsEmbeddingProvider::boxed(
                OllamaEmbeddingModel::try_new(&base_url, model, dims)?,
            ))
        }
        "openai" => Ok(TinyAgentsEmbeddingProvider::boxed(openai_model(
            "https://api.openai.com",
            api_key,
            model,
            dims,
            true,
        ))),
        "cohere" => Ok(TinyAgentsEmbeddingProvider::boxed(
            CohereEmbeddingModel::new(api_key)
                .with_model(model)
                .with_dimensions(dims),
        )),
        "custom" => {
            let url = custom_endpoint.unwrap_or("");
            custom_openai_provider(url, api_key, model, dims)
        }
        name if name.starts_with("custom:") => {
            let url = custom_endpoint.unwrap_or_else(|| name.strip_prefix("custom:").unwrap_or(""));
            custom_openai_provider(url, api_key, model, dims)
        }
        "none" => Ok(TinyAgentsEmbeddingProvider::boxed(NoopEmbeddingModel)),
        unknown => Err(anyhow::anyhow!(
            "unknown embedding provider: \"{unknown}\". \
             Supported: \"managed\", \"voyage\", \"openai\", \"cohere\", \
             \"ollama\", \"custom\", \"none\""
        )),
    }
}

/// Config-aware variant of [`create_embedding_provider_with_credentials`].
///
/// Behaves identically for every provider **except** `managed`/`cloud`. For
/// those it threads the caller's real credential-store location
/// ([`managed_credential_scope`]) into the cloud embedder's bearer resolver — the
/// same `(state_dir, encrypt)` pair
/// [`AuthService::from_config`](crate::security::credentials::AuthService::from_config)
/// uses to **store** the `app-session` token at sign-in.
///
/// The keyless constructors hardcode `(None, true)`, which resolves to
/// `default_state_dir()` (`~/.openhuman` root) with encryption forced on. On a
/// shipped desktop `OPENHUMAN_WORKSPACE` is unset and the session token lives
/// under the user-scoped `~/.openhuman/users/<uid>/auth-profiles.json`, so that
/// hardcode reads the *wrong* file and a signed-in user's managed "Test
/// connection" / embed falsely reports "No backend session" (#5356). Callers
/// that hold a `&Config` must route managed construction through here.
pub fn create_embedding_provider_with_config(
    config: &Config,
    provider: &str,
    model: &str,
    dims: usize,
    api_key: &str,
    custom_endpoint: Option<&str>,
) -> anyhow::Result<Box<dyn EmbeddingProvider>> {
    match provider {
        "cloud" | "managed" => {
            let (state_dir, encrypt_secrets) = managed_credential_scope(config);
            // Never log `state_dir`: the user-scoped path embeds the OS username
            // and/or `users/<uid>` (PII). Log only the non-identifying flag.
            log::debug!(
                "[embeddings::factory] building managed embedder from config credential scope (encrypt={encrypt_secrets})"
            );
            Ok(Box::new(OpenHumanCloudEmbedding::new(
                None,
                state_dir,
                encrypt_secrets,
                model,
                dims,
            )))
        }
        // Ollama must use the config-aware URL so a custom `local_ai.base_url`
        // is honoured — the credential-store path calls `ollama_base_url()`
        // (env-only) and diverges when the setting is set (#6032).
        "ollama" => {
            let base_url = tinyinference::local::ollama::ollama_base_url_from_override(
                config.local_ai.base_url.as_deref(),
            );
            Ok(TinyAgentsEmbeddingProvider::boxed(
                OllamaEmbeddingModel::try_new(&base_url, model, dims)?,
            ))
        }
        // Every other provider is credential-store-agnostic (BYO key or local
        // endpoint), so the existing construction is correct unchanged.
        other => {
            create_embedding_provider_with_credentials(other, model, dims, api_key, custom_endpoint)
        }
    }
}

/// The `(state_dir, encrypt)` the managed/cloud embedder must use to find the
/// `app-session` token. Delegates to
/// [`state_dir_from_config`](crate::security::credentials::state_dir_from_config)
/// — the exact helper [`AuthService::from_config`] uses — so the embedder reads
/// the token from the **same** store sign-in wrote it to, including the
/// `"."`-fallback when `config_path` has no parent (a bare filename). Returning
/// the raw parent instead would yield `None` there and silently fall back to
/// `default_state_dir()` — the very divergence this fix removes. Extracted as a
/// pure fn so the #5356 invariant is unit-testable without a network round-trip.
fn managed_credential_scope(config: &Config) -> (Option<PathBuf>, bool) {
    use crate::security::credentials::state_dir_from_config;
    (Some(state_dir_from_config(config)), config.secrets.encrypt)
}

/// Returns the default embedding provider — cloud (OpenHuman backend, Voyage) —
/// scoped to `config`'s credential store.
///
/// This is the [`default_embedding_provider`] every caller that holds a
/// `&Config` must use. It threads the caller's real credential-store location
/// ([`managed_credential_scope`]) into the cloud embedder's bearer resolver — the
/// same `(state_dir, encrypt)` pair sign-in wrote the `app-session` token to — so
/// a signed-in user's ingest/seal embeds read the session they actually have.
///
/// The config-less [`default_embedding_provider`] hardcodes `(None, true)` and so
/// resolves `default_state_dir()` with encryption forced on; that only lands on
/// the right store for a default-root, encrypted, single-user install. Routing
/// the memory client's inline embedder through the keyless constructor is what
/// made a signed-in user's ingested documents persist vector-less — "Test
/// connection" passed (config-scoped) while the embed batch silently failed
/// (keyless scope) — #5501.
pub fn default_embedding_provider_with_config(config: &Config) -> Arc<dyn EmbeddingProvider> {
    // Keep the stored value for credential lookup. Credentials are keyed by
    // the provider value that settings persisted; trimming before lookup can
    // select a different credential. Normalize only the construction input.
    let stored_provider = config.memory.embedding_provider.as_str();
    let provider = stored_provider.trim();
    if !provider.is_empty()
        && !provider.eq_ignore_ascii_case("cloud")
        && !provider.eq_ignore_ascii_case("managed")
    {
        let (provider_slug, raw_custom_endpoint) = match provider.strip_prefix("custom:") {
            Some(endpoint) => ("custom", Some(endpoint)),
            None if provider == "custom" => ("custom", None),
            None => (provider, None),
        };
        let api_key = super::rpc::resolve_api_key(config, provider_slug);
        let custom_endpoint = match raw_custom_endpoint {
            Some(endpoint) => validate_custom_endpoint(endpoint, !api_key.is_empty()).map(Some),
            None if provider_slug == "custom" => Err(anyhow::anyhow!(
                "custom embedding provider endpoint is missing"
            )),
            None => Ok(None),
        };
        let requires_key = matches!(provider_slug, "voyage" | "openai" | "cohere")
            || (provider_slug == "custom" && raw_custom_endpoint.is_none());
        if let Ok(custom_endpoint) = custom_endpoint {
            match create_embedding_provider_with_config(
                config,
                provider_slug,
                &config.memory.embedding_model,
                config.memory.embedding_dimensions,
                &api_key,
                custom_endpoint.as_deref(),
            ) {
                Ok(provider) => {
                    if !requires_key || !api_key.is_empty() {
                        return Arc::from(provider);
                    }
                    log::warn!(
                        "[embeddings::factory] configured embedding provider has no stored credential (kind={provider_slug}); falling back to managed cloud embedder"
                    );
                }
                Err(_) => {
                    let kind = match provider {
                        "voyage" | "openai" | "cohere" | "ollama" | "none" => provider,
                        _ if provider.starts_with("custom") => "custom",
                        _ => "unknown",
                    };
                    log::warn!(
                        "[embeddings::factory] configured embedding provider failed to build (kind={kind}); falling back to managed cloud embedder"
                    );
                }
            }
        } else {
            log::warn!(
                "[embeddings::factory] configured custom embedding provider has no valid endpoint; falling back to managed cloud embedder"
            );
        }
    }
    let (state_dir, encrypt_secrets) = managed_credential_scope(config);
    // Never log `state_dir`: the user-scoped path embeds the OS username and/or
    // `users/<uid>` (PII). Log only the non-identifying flag.
    log::debug!(
        "[embeddings::factory] building default managed embedder from config credential scope (encrypt={encrypt_secrets})"
    );
    Arc::new(OpenHumanCloudEmbedding::new(
        None,
        state_dir,
        encrypt_secrets,
        DEFAULT_CLOUD_EMBEDDING_MODEL,
        DEFAULT_CLOUD_EMBEDDING_DIMENSIONS,
    ))
}

/// Returns the default embedding provider — cloud (OpenHuman backend, Voyage).
///
/// The cloud embedder lazily resolves the session JWT and API URL on each
/// call, so this can be constructed before login completes; the first
/// `embed()` will fail with a clear message if the user is unauthenticated.
///
/// **Keyless — prefer [`default_embedding_provider_with_config`].** This hardcodes
/// `(None, true)` for the credential scope, resolving `default_state_dir()`
/// (`~/.openhuman` root, or `users/<active>` post-#5427) with encryption forced
/// on. That reads the wrong store whenever the caller's config disables secret
/// encryption or roots the workspace/user elsewhere than the process default
/// (#5356 / #5501). Only callers that genuinely hold no `&Config` should use it.
pub fn default_embedding_provider() -> Arc<dyn EmbeddingProvider> {
    Arc::new(OpenHumanCloudEmbedding::new(
        None,
        None,
        true,
        DEFAULT_CLOUD_EMBEDDING_MODEL,
        DEFAULT_CLOUD_EMBEDDING_DIMENSIONS,
    ))
}

/// Returns the local Ollama-backed embedding provider. Only used when the
/// caller has explicitly opted into local-only embeddings.
pub fn default_local_embedding_provider() -> Arc<dyn EmbeddingProvider> {
    Arc::new(TinyAgentsEmbeddingProvider::new(
        OllamaEmbeddingModel::default(),
    ))
}

#[cfg(test)]
#[path = "factory_tests.rs"]
mod tests;
