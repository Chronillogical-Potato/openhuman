//! The SDK-backed [`BackendTransport`]: every hosted-backend request the core
//! makes rides the vendored `tinyhumans-sdk`'s HTTP primitive, which owns
//! route policy (the unexposed-route registry) and the credential header
//! shapes the backend expects.

use std::sync::Arc;

use async_trait::async_trait;
use openhuman_core::api::headers::build_backend_client;
use openhuman_core::api::transport::{
    BackendRequest, BackendTransport, BackendTransportError, TransportProfile,
};
use openhuman_core::security::credentials::session_support::BackendCredential;
use serde_json::Value;
use tinyhumans_sdk::TinyHumansClient;

mod error;

pub use error::map_sdk_error;

/// One `reqwest::Client` per [`TransportProfile`], each built from the core's
/// [`backend_client_builder`](openhuman_core::api::headers::backend_client_builder)
/// so TLS, timeouts and the attribution headers (`x-core-version`,
/// `x-tauri-version`, `x-sdk-name`) are exactly what the core specifies.
pub struct SdkBackendTransport {
    api: reqwest::Client,
    integrations: reqwest::Client,
}

impl SdkBackendTransport {
    /// Build the transport. Fails only if a `reqwest::Client` cannot be
    /// constructed (TLS backend unavailable, malformed attribution header).
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            api: build_backend_client(TransportProfile::Api)?,
            integrations: build_backend_client(TransportProfile::Integrations)?,
        })
    }

    /// [`Self::new`] as a shared handle ready for
    /// [`install_backend_transport`](openhuman_core::api::transport::install_backend_transport).
    pub fn shared() -> anyhow::Result<Arc<dyn BackendTransport>> {
        Ok(Arc::new(Self::new()?))
    }

    fn client(&self, profile: TransportProfile) -> &reqwest::Client {
        match profile {
            TransportProfile::Api => &self.api,
            TransportProfile::Integrations => &self.integrations,
        }
    }

    fn sdk(&self, req: &BackendRequest<'_>) -> TinyHumansClient {
        // The product identity also rides on the SDK's own default headers,
        // not just the transport's, so it survives if the SDK is ever given a
        // client this crate did not build. The SDK applies its own headers
        // after these, so it cannot be clobbered by `x-sdk-client`.
        let sdk = TinyHumansClient::new(req.base_url)
            .with_http_client(self.client(req.profile).clone())
            .with_default_headers(openhuman_core::api::product::product_identity_headers());
        match req.credential {
            Some(BackendCredential::Session(secret)) => {
                sdk.with_token(Some(secret.trim().to_string()))
            }
            Some(BackendCredential::ApiKey(secret)) => {
                log::trace!("[tinyhumans-transport] authenticating request with x-api-key");
                sdk.with_api_key(Some(secret.trim().to_string()))
            }
            None => sdk,
        }
    }
}

#[async_trait]
impl BackendTransport for SdkBackendTransport {
    async fn send_json(&self, req: BackendRequest<'_>) -> Result<Value, BackendTransportError> {
        log::trace!(
            "[tinyhumans-transport] {} {} profile={:?} unwrap={}",
            req.method,
            req.path,
            req.profile,
            req.unwrap_envelope
        );
        let sdk = self.sdk(&req);
        sdk.raw()
            .send(
                req.method.clone(),
                req.path,
                req.query,
                req.body,
                req.unwrap_envelope,
            )
            .await
            .map_err(map_sdk_error)
    }

    async fn send_multipart(
        &self,
        req: BackendRequest<'_>,
        form: reqwest::multipart::Form,
    ) -> Result<Value, BackendTransportError> {
        log::trace!(
            "[tinyhumans-transport] POST(multipart) {} profile={:?}",
            req.path,
            req.profile
        );
        let sdk = self.sdk(&req);
        sdk.raw()
            .post_multipart(req.path, form)
            .await
            .map_err(map_sdk_error)
    }

    fn http_client(&self, profile: TransportProfile) -> reqwest::Client {
        self.client(profile).clone()
    }

    fn name(&self) -> &'static str {
        "tinyhumans-sdk"
    }
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
