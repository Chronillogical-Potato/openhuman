//! Crate-native Anthropic Messages API client construction.
//!
//! A `cloud_providers` entry with `auth_style = "anthropic"` used to be served
//! by the crate `OpenAiModel` in its Anthropic-auth flavour — Chat Completions
//! against Anthropic's OpenAI-compatibility endpoint. That endpoint documents
//! prompt caching as **unsupported** and reports `prompt_tokens_details` as
//! always empty, so every turn on a BYOK Claude key re-billed the entire system
//! prompt, tool catalogue, and conversation at full price, and the host's
//! `[cache]` diagnostics could never show a hit.
//!
//! This module is the boundary where that entry becomes the crate's native
//! [`AnthropicModel`] instead: the Messages API with `cache_control`
//! breakpoints on the tool set, the system prompt, and the tail of the
//! conversation (see the crate adapter's docs for the placement rule), native
//! `tool_use` / `tool_result` blocks, signed thinking replay, and SSE streaming.
//! The harness's `PromptCacheSegmentMiddleware` already declares the stable
//! prefix on every request; this is what turns those declarations into wire
//! markers on the one provider that needs them explicitly.

use std::sync::Arc;

use tinyinference::model::ChatModel;
use tinyinference::providers::anthropic::AnthropicModel;

/// Whether an endpoint is known to speak the Anthropic Messages API.
///
/// Authentication style is not sufficient to select the wire protocol:
/// existing configurations may use an Anthropic key with an OpenAI-compatible
/// proxy. Keep those endpoints on Chat Completions unless the endpoint is the
/// first-party Messages API.
pub(crate) fn endpoint_is_anthropic_messages(endpoint: &str) -> bool {
    crate::config::schema::cloud_providers::endpoint_host(endpoint)
        .is_some_and(|host| host == "api.anthropic.com")
}

/// The resolved config for one Anthropic Messages API provider.
pub(crate) struct CrateAnthropicConfig<'a> {
    /// Base URL (`https://api.anthropic.com/v1` for the hosted API; a
    /// Messages-compatible proxy otherwise). `/messages` is appended unless
    /// the URL already ends in it.
    pub endpoint: &'a str,
    /// API credential, sent as `x-api-key`.
    pub api_key: &'a str,
    /// Default model id baked onto the client (a per-call `ModelRequest.model`
    /// still overrides it).
    pub model: &'a str,
    /// Fixed temperature override for every call, when set (the `@<temp>`
    /// provider-string suffix).
    pub temperature_override: Option<f64>,
    /// Model-id `*`-glob patterns whose targets reject a `temperature` param.
    pub temperature_unsupported_models: &'a [String],
}

/// Build a crate-native [`AnthropicModel`] (`ChatModel`) for the given config.
pub(crate) fn build_crate_anthropic_model(
    config: CrateAnthropicConfig<'_>,
) -> Arc<dyn ChatModel<()>> {
    log::debug!(
        "[providers][chat-factory] building native Anthropic Messages client endpoint={} model={} temperature_override={:?}",
        config.endpoint,
        config.model,
        config.temperature_override
    );
    let model = AnthropicModel::with_base_url(config.api_key, config.endpoint)
        .with_model(config.model)
        .with_temperature_override(config.temperature_override)
        .with_temperature_unsupported_models(config.temperature_unsupported_models.iter().cloned());
    Arc::new(model)
}

#[cfg(test)]
#[path = "crate_anthropic_tests.rs"]
mod tests;
