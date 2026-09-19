//! Wire-shape parity for the SDK transport: what the core's classification
//! code relies on, pinned against a mock backend.

use super::*;
use openhuman_core::api::transport::{clear_backend_transport, resolve_backend_transport};
use serde_json::json;
use wiremock::matchers::{header, header_exists, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn request<'a>(
    base: &'a str,
    method: reqwest::Method,
    path: &'a str,
    credential: Option<&'a BackendCredential>,
    unwrap: bool,
) -> BackendRequest<'a> {
    BackendRequest {
        profile: TransportProfile::Api,
        base_url: base,
        method,
        path,
        query: &[],
        body: None,
        credential,
        unwrap_envelope: unwrap,
    }
}

#[tokio::test]
async fn session_credential_rides_bearer_with_sdk_and_product_headers() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/teams/me/usage"))
        .and(header("authorization", "Bearer jwt.a.b"))
        .and(header("x-sdk-client", "tinyhumans-rust"))
        .and(header("x-sdk-name", "openhuman"))
        .and(header_exists("x-core-version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "data": {"remainingUsd": 1.5}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let transport = SdkBackendTransport::new().unwrap();
    let credential = BackendCredential::Session("jwt.a.b\n".into());
    let value = transport
        .send_json(request(
            &server.uri(),
            reqwest::Method::GET,
            "/teams/me/usage",
            Some(&credential),
            true,
        ))
        .await
        .unwrap();
    assert_eq!(value, json!({"remainingUsd": 1.5}));
}

#[tokio::test]
async fn api_key_credential_rides_x_api_key_and_never_authorization() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/payments/summary"))
        .and(header("x-api-key", "th_key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .expect(1)
        .mount(&server)
        .await;

    let transport = SdkBackendTransport::new().unwrap();
    let credential = BackendCredential::ApiKey("th_key".into());
    let value = transport
        .send_json(request(
            &server.uri(),
            reqwest::Method::GET,
            "/payments/summary",
            Some(&credential),
            true,
        ))
        .await
        .unwrap();
    assert_eq!(value, json!({"ok": true}));
    let received = server.received_requests().await.unwrap();
    assert!(received[0].headers.get("authorization").is_none());
}

#[tokio::test]
async fn non_2xx_maps_to_status_with_parsed_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/x"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({"error": "Invalid token"})))
        .mount(&server)
        .await;

    let transport = SdkBackendTransport::new().unwrap();
    let err = transport
        .send_json(request(&server.uri(), reqwest::Method::GET, "/x", None, true))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        BackendTransportError::Status { status: 401, ref body } if body == &json!({"error": "Invalid token"})
    ));
}

#[tokio::test]
async fn failed_envelope_on_2xx_maps_to_envelope_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/y"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": false,
            "error": "Toolkit X is not enabled",
            "errorCode": "TOOLKIT_DISABLED"
        })))
        .mount(&server)
        .await;

    let transport = SdkBackendTransport::new().unwrap();
    let err = transport
        .send_json(request(&server.uri(), reqwest::Method::POST, "/y", None, true))
        .await
        .unwrap_err();
    match err {
        BackendTransportError::Envelope {
            error, error_code, ..
        } => {
            assert_eq!(error, "Toolkit X is not enabled");
            assert_eq!(error_code.as_deref(), Some("TOOLKIT_DISABLED"));
        }
        other => panic!("expected Envelope, got {other:?}"),
    }
}

#[tokio::test]
async fn raw_envelope_is_returned_when_not_unwrapping() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/agent-integrations/pricing"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"success": true, "data": {"a": 1}})),
        )
        .mount(&server)
        .await;

    let transport = SdkBackendTransport::new().unwrap();
    let value = transport
        .send_json(request(
            &server.uri(),
            reqwest::Method::GET,
            "/agent-integrations/pricing",
            None,
            false,
        ))
        .await
        .unwrap();
    assert_eq!(value, json!({"success": true, "data": {"a": 1}}));
}

#[tokio::test]
async fn sdk_route_policy_refuses_unexposed_routes_before_sending() {
    let server = MockServer::start().await;
    let transport = SdkBackendTransport::new().unwrap();
    let err = transport
        .send_json(request(
            &server.uri(),
            reqwest::Method::POST,
            "/admin/coupons",
            None,
            true,
        ))
        .await
        .unwrap_err();
    assert!(matches!(err, BackendTransportError::RouteNotExposed(m, p) if m == "POST" && p.contains("admin")));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn install_makes_the_core_resolve_this_transport() {
    // Serialised through the install slot; the core's global is process state.
    let _t = crate::install(crate::InstallOptions::default()).unwrap();
    assert!(crate::is_installed());
    assert_eq!(resolve_backend_transport().unwrap().name(), "tinyhumans-sdk");
    clear_backend_transport();
    assert!(!crate::is_installed());
    // Re-install after a clear restores the same transport.
    let _t = crate::install(crate::InstallOptions::default()).unwrap();
    assert_eq!(resolve_backend_transport().unwrap().name(), "tinyhumans-sdk");
}

#[tokio::test]
async fn backend_client_round_trips_through_the_installed_transport() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/announcements/latest"))
        .and(header("authorization", "Bearer jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "data": {"id": "a1"}
        })))
        .mount(&server)
        .await;

    let _t = crate::install(crate::InstallOptions::default()).unwrap();
    let client = openhuman_core::api::BackendOAuthClient::new(&server.uri()).unwrap();
    let value = client
        .authed_json("jwt", reqwest::Method::GET, "/announcements/latest", None)
        .await
        .unwrap();
    assert_eq!(value, json!({"id": "a1"}));
}
