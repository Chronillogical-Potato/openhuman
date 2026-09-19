//! **Transitional (P1 shim, removed once the SDK leaves the core).**
//!
//! Until every host installs a transport from `openhuman-tinyhumans`, the
//! core keeps its own SDK-backed [`BackendTransport`] as the last-resort
//! fallback in [`resolve_backend_transport`](super::resolve_backend_transport)
//! so the desktop shell, TUI, CLI and the mock-backend E2E suites keep working
//! unchanged while the migration lands phase by phase. It is byte-for-byte the
//! implementation `openhuman-tinyhumans` ships; when the last host is
//! switched, delete this module and the `tinyhumans-sdk` dependency together.

use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use serde_json::Value;
use tinyhumans_sdk::{Error as SdkError, TinyHumansClient};

use super::{BackendRequest, BackendTransport, BackendTransportError, TransportProfile};
use crate::security::credentials::session_support::BackendCredential;

pub struct SdkCompatTransport {
    api: reqwest::Client,
    integrations: reqwest::Client,
}

impl SdkCompatTransport {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            api: crate::api::headers::build_backend_client(TransportProfile::Api)?,
            integrations: crate::api::headers::build_backend_client(
                TransportProfile::Integrations,
            )?,
        })
    }

    /// The process-wide shim instance, built on first use.
    pub fn shared() -> Option<Arc<dyn BackendTransport>> {
        static SHARED: OnceLock<Option<Arc<SdkCompatTransport>>> = OnceLock::new();
        SHARED
            .get_or_init(|| match Self::new() {
                Ok(t) => Some(Arc::new(t)),
                Err(e) => {
                    log::error!("[backend-transport] sdk shim unavailable: {e:#}");
                    None
                }
            })
            .clone()
            .map(|t| t as Arc<dyn BackendTransport>)
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
            .with_default_headers(crate::api::product::product_identity_headers());
        match req.credential {
            Some(BackendCredential::Session(secret)) => {
                sdk.with_token(Some(secret.trim().to_string()))
            }
            Some(BackendCredential::ApiKey(secret)) => {
                log::trace!("[backend-transport] authenticating request with x-api-key");
                sdk.with_api_key(Some(secret.trim().to_string()))
            }
            None => sdk,
        }
    }
}

/// Map the SDK's error onto the core-owned transport error.
pub fn map_sdk_error(error: SdkError) -> BackendTransportError {
    match error {
        SdkError::Url(e) => BackendTransportError::Url(e.to_string()),
        SdkError::Http(e) => BackendTransportError::Http(e),
        SdkError::Status { status, body } => BackendTransportError::Status { status, body },
        SdkError::Header(e) => BackendTransportError::Header(e.to_string()),
        SdkError::Decode(e) => BackendTransportError::Decode(e.to_string()),
        SdkError::RouteNotExposed(method, path) => {
            BackendTransportError::RouteNotExposed(method, path)
        }
        SdkError::Envelope {
            error,
            error_code,
            details,
        } => BackendTransportError::Envelope {
            error,
            error_code,
            details,
        },
        other => BackendTransportError::Other(other.to_string()),
    }
}

#[async_trait]
impl BackendTransport for SdkCompatTransport {
    async fn send_json(&self, req: BackendRequest<'_>) -> Result<Value, BackendTransportError> {
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
        "tinyhumans-sdk (core shim)"
    }
}
