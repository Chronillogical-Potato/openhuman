//! Test-only `reqwest` transport so the core's wiremock unit tests exercise
//! [`BackendOAuthClient`](crate::api::rest::BackendOAuthClient) and
//! `IntegrationClient` without any host crate. Never compiled into a
//! production build: [`resolve_backend_transport`](super::resolve_backend_transport)
//! only reaches it under `cfg(test)`.
//!
//! Deliberately *not* a copy of the SDK transport: it applies no route policy
//! and sends no `x-sdk-client`. The `openhuman-tinyhumans` crate carries a
//! parity test pinning the headers and envelope behaviour both share.

use std::sync::Arc;

use async_trait::async_trait;
use reqwest::header::CONTENT_TYPE;
use reqwest::Method;
use serde_json::Value;

use super::{
    compose_url, credential_headers, parse_body_text, unwrap_envelope, BackendRequest,
    BackendTransport, BackendTransportError, TransportProfile,
};

pub struct PlainHttpTransport {
    api: reqwest::Client,
    integrations: reqwest::Client,
}

impl PlainHttpTransport {
    pub fn new() -> Self {
        Self {
            api: crate::api::headers::build_backend_client(TransportProfile::Api)
                .expect("plain api client"),
            integrations: crate::api::headers::build_backend_client(TransportProfile::Integrations)
                .expect("plain integrations client"),
        }
    }

    /// A fresh instance. Deliberately *not* cached: tests change the product
    /// identity between calls and expect the next client to carry it, and
    /// building two `reqwest::Client`s is cheap.
    pub fn fresh() -> Arc<dyn BackendTransport> {
        Arc::new(Self::new())
    }

    fn client(&self, profile: TransportProfile) -> &reqwest::Client {
        match profile {
            TransportProfile::Api => &self.api,
            TransportProfile::Integrations => &self.integrations,
        }
    }

    async fn finish(
        response: reqwest::Response,
        unwrap: bool,
    ) -> Result<Value, BackendTransportError> {
        let status = response.status();
        let value = parse_body_text(response.text().await?);
        if !status.is_success() {
            return Err(BackendTransportError::Status {
                status: status.as_u16(),
                body: value,
            });
        }
        if unwrap {
            unwrap_envelope(value)
        } else {
            Ok(value)
        }
    }
}

impl Default for PlainHttpTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BackendTransport for PlainHttpTransport {
    async fn send_json(&self, req: BackendRequest<'_>) -> Result<Value, BackendTransportError> {
        let url = compose_url(req.base_url, req.path, req.query)?;
        // Product identity is stamped per request, as the SDK transport does,
        // so a change after this client was built still reaches the wire.
        let mut request = self
            .client(req.profile)
            .request(req.method, url)
            .headers(crate::api::product::product_identity_headers())
            .header(reqwest::header::ACCEPT, "application/json")
            .header(CONTENT_TYPE, "application/json");
        if let Some(credential) = req.credential {
            request = request.headers(credential_headers(credential)?);
        }
        if let Some(body) = req.body {
            request = request.json(body);
        }
        Self::finish(request.send().await?, req.unwrap_envelope).await
    }

    async fn send_multipart(
        &self,
        req: BackendRequest<'_>,
        form: reqwest::multipart::Form,
    ) -> Result<Value, BackendTransportError> {
        let url = compose_url(req.base_url, req.path, &[])?;
        let mut request = self
            .client(req.profile)
            .request(Method::POST, url)
            .headers(crate::api::product::product_identity_headers())
            .header(reqwest::header::ACCEPT, "application/json");
        if let Some(credential) = req.credential {
            request = request.headers(credential_headers(credential)?);
        }
        Self::finish(request.multipart(form).send().await?, true).await
    }

    fn http_client(&self, profile: TransportProfile) -> reqwest::Client {
        self.client(profile).clone()
    }

    fn name(&self) -> &'static str {
        "plain-test"
    }
}
