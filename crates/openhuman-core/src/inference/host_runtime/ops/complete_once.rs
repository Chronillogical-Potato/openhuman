//! One stateless model call on an explicit route: no session, no tools, no
//! prompt guard.
//!
//! [`agent_chat_simple`](super::agent_chat_simple) is the closest neighbour,
//! and the differences are the point of this module:
//!
//! - **No prompt guard.** `enforce_user_prompt_or_reject` exists to stop a
//!   user steering an agent that holds tools. A completion holds none, and its
//!   callers (code reviewers, classifiers, extractors) routinely feed it
//!   adversarial text *as data* — a guard would reject exactly the inputs they
//!   exist to read.
//! - **The caller's request, not a message string.** System/user roles, images,
//!   `response_format`, `max_tokens` and pass-through `provider_options` all
//!   reach the wire, and the whole [`ModelResponse`] (finish reason, usage, raw
//!   provider body) comes back.
//! - **Tools are refused, not ignored.** A request that declares tools is an
//!   error: a host that wants a tool loop wants an agent turn, and silently
//!   dropping the declarations would hide that mistake.
//! - **The route is mandatory.** The call never falls back to the account's
//!   configured provider, so the model a caller pays for is the one it named.

use tinyinference_llm::model::{ModelRequest, ModelResponse};

use crate::config::schema::EphemeralRoute;
use crate::config::Config;
use crate::inference::provider as providers;

/// Run `request` once against `route` and return the provider's response.
///
/// `request.model` is required: it names the model on `route`'s endpoint and
/// is what the route pins every chat role to. Errors are rendered strings, the
/// same contract as the other `ops` functions, so the facade maps them in one
/// place.
pub async fn complete_once(
    route: EphemeralRoute,
    request: ModelRequest,
) -> Result<ModelResponse, String> {
    if !request.tools.is_empty() {
        return Err(
            "complete_once: tools are not supported on a stateless completion; use an agent turn"
                .to_string(),
        );
    }
    let model_id = request
        .model
        .as_deref()
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .ok_or_else(|| "complete_once: request.model is required".to_string())?
        .to_string();

    // A throwaway in-memory config: nothing here reads or writes the user's
    // workspace, and the route below replaces every chat role, so defaults are
    // only the scaffolding the provider factory expects.
    let mut config = Config {
        default_model: Some(model_id.clone()),
        ..Config::default()
    };
    crate::config::schema::ephemeral_route::apply(&mut config, route);

    let temperature = request.temperature.unwrap_or(config.default_temperature);
    let (model, resolved_model) =
        providers::create_chat_model_with_model_id("chat", &config, temperature)
            .map_err(|e| format!("complete_once: {e}"))?;
    tracing::debug!(
        requested_model = %model_id,
        resolved_model = %resolved_model,
        messages = request.messages.len(),
        response_format = request.response_format.is_some(),
        max_tokens = ?request.max_tokens,
        "[inference] complete_once invoking chat model"
    );

    model
        .invoke(&(), request)
        .await
        .map_err(|e| format!("complete_once: {e}"))
}

#[cfg(test)]
#[path = "complete_once_tests.rs"]
mod tests;
