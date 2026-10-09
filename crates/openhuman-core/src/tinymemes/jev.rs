//! Jev as OpenHuman manages it.
//!
//! tinymemes uses the OpenHuman-managed Jev: a signed-in TinyHumans session
//! reaches Jev through the backend's System One proxy, billed to the account.
//! There is no other OpenHuman-side route. A different Jev is only used when
//! `TINYMEMES_JEV` (and its key variables) override it in the environment.
//!
//! `None` means managed Jev is unavailable (signed out, or the offline local
//! session, which has no backend account); the caller then answers Jev's
//! questions with OpenHuman's own LLM.

use std::sync::Arc;

use tinymemes::decisions::{Client, ClientConfig};

use crate::config::Config;
use crate::security::credentials::session_support::{
    is_local_session_token, resolve_backend_credential, BackendCredential,
};

pub(crate) fn managed(config: &Config) -> Option<Arc<dyn tinymemes::Evaluator>> {
    // Only a session token can be the offline local session; an API key is
    // never classified by its shape.
    let secret = match resolve_backend_credential(config).ok()? {
        BackendCredential::Session(token) if is_local_session_token(&token) => return None,
        BackendCredential::Session(token) | BackendCredential::ApiKey(token) => token,
    };
    let base = crate::backend::base_url(&config.api_url).ok()?;
    let mut client = ClientConfig::tinyhumans_openrouter(secret);
    client.base_url = base;
    if let Some(identity) = crate::backend::product_identity() {
        client = client.with_sdk_name(&identity);
    }
    match Client::new(client) {
        Ok(c) => Some(Arc::new(c)),
        Err(e) => {
            log::warn!("[tinymemes] managed jev client build failed: {e}");
            None
        }
    }
}
