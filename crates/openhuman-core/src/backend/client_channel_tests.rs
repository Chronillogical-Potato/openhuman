use super::*;

#[tokio::test]
async fn channel_routes_build_expected_requests_and_validate_inputs() {
    type Requests = Arc<Mutex<Vec<(String, String, Value)>>>;
    async fn capture(
        State(seen): State<Requests>,
        request: axum::http::Request<axum::body::Body>,
    ) -> Json<Value> {
        let method = request.method().to_string();
        let path = request.uri().to_string();
        let bytes = axum::body::to_bytes(request.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        seen.lock().unwrap().push((method, path, body));
        Json(json!({ "success": true, "data": { "ok": true } }))
    }

    let seen = Requests::default();
    let app = Router::new().fallback(capture).with_state(seen.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let client = BackendClient::new(&format!("http://{addr}")).unwrap();

    client
        .send_channel_message(" /telegram/ ", "jwt", json!({ "text": "hello" }))
        .await
        .unwrap();
    client
        .send_channel_reaction("telegram", "jwt", json!({ "emoji": "👍" }))
        .await
        .unwrap();
    client
        .create_channel_thread("telegram", "jwt", "  topic  ")
        .await
        .unwrap();
    client
        .update_channel_thread("telegram", "jwt", "thread-1", "close")
        .await
        .unwrap();
    client
        .list_channel_threads("telegram", "jwt", Some(true))
        .await
        .unwrap();
    client
        .list_channel_threads("telegram", "jwt", Some(false))
        .await
        .unwrap();
    client
        .list_channel_threads("telegram", "jwt", None)
        .await
        .unwrap();

    let requests = seen.lock().unwrap().clone();
    assert_eq!(
        requests[0],
        (
            "POST".into(),
            "/channels/telegram/messages".into(),
            json!({ "text": "hello" })
        )
    );
    assert_eq!(
        requests[1],
        (
            "POST".into(),
            "/channels/telegram/reactions".into(),
            json!({ "emoji": "👍" })
        )
    );
    assert_eq!(
        requests[2],
        (
            "POST".into(),
            "/channels/telegram/threads".into(),
            json!({ "title": "topic" })
        )
    );
    assert_eq!(
        requests[3],
        (
            "PATCH".into(),
            "/channels/telegram/threads/thread-1".into(),
            json!({ "action": "close" })
        )
    );
    assert_eq!(requests[4].1, "/channels/telegram/threads?active=true");
    assert_eq!(requests[5].1, "/channels/telegram/threads?active=false");
    assert_eq!(requests[6].1, "/channels/telegram/threads");
    drop(requests);

    assert!(client
        .send_channel_message(" / ", "jwt", json!({}))
        .await
        .is_err());
    assert!(client
        .send_channel_reaction(" ", "jwt", json!({}))
        .await
        .is_err());
    assert!(client
        .create_channel_thread("telegram", "jwt", " ")
        .await
        .is_err());
    assert!(client
        .update_channel_thread("telegram", "jwt", " ", "close")
        .await
        .is_err());
    assert!(client
        .update_channel_thread("telegram", "jwt", "thread-1", "delete")
        .await
        .is_err());
    assert!(client.list_channel_threads("", "jwt", None).await.is_err());
}

#[tokio::test]
async fn authed_json_reports_non_channel_404_still_propagates() {
    // TAURI-RUST-8C: a GET 404 on a non-channel path (e.g. `/teams/me/usage`)
    // falls through to `report_error` (not a typed/suppressed state) — it must
    // still return an Err (no suppression) and not a typed `BackendApiError`.
    let app = Router::new().route(
        "/teams/me/usage",
        get(|| async {
            (
                axum::http::StatusCode::NOT_FOUND,
                r#"{"message":"Not Found"}"#,
            )
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let base_url = format!("http://{addr}");
    let client = BackendClient::new(&base_url).unwrap();

    let err = client
        .authed_json("mock-jwt", Method::GET, "/teams/me/usage", None)
        .await
        .unwrap_err();
    assert!(err.downcast_ref::<BackendApiError>().is_none());
    let msg = format!("{err:#}");
    assert!(msg.contains("404"), "error should carry the status: {msg}");
    assert!(
        msg.contains("/teams/me/usage"),
        "error should carry the path: {msg}"
    );
}

#[test]
fn flatten_authed_error_maps_unauthorized_to_session_expired_sentinel() {
    // #3297: the typed `Unauthorized` (expected session-lapse 401) must flatten
    // onto a string that the JSON-RPC session-expiry classifiers recognise, so
    // it is suppressed from Sentry (TAURI-RUST-8WY / 8WZ) instead of leaking.
    let err = anyhow::Error::new(BackendApiError::Unauthorized {
        method: "GET".to_string(),
        path: "/teams/me/usage".to_string(),
    });
    let flat = flatten_authed_error(err);

    // Carries the SESSION_EXPIRED sentinel + preserves method/path for logs.
    assert!(
        flat.contains("SESSION_EXPIRED"),
        "expected sentinel, got: {flat}"
    );
    assert!(flat.contains("GET"), "method preserved: {flat}");
    assert!(flat.contains("/teams/me/usage"), "path preserved: {flat}");

    // Contract cross-check: the flattened string MUST classify as session
    // expiry. This couples the mapping to the actual classifier — if either the
    // sentinel or the classifier drifts, this fails instead of silently leaking.
    assert!(
        crate::core::observability::is_session_expired_message(&flat),
        "flattened Unauthorized must classify as session expiry: {flat}"
    );
}

#[test]
fn flatten_authed_error_preserves_non_unauthorized_chain() {
    // A non-Unauthorized failure (e.g. a transient network/timeout error) keeps
    // its full `{e:#}` anyhow chain and must NOT be demoted to session expiry —
    // genuine failures still reach Sentry.
    let err = anyhow::anyhow!("connect timeout").context("backend request GET /teams/me/usage");
    let flat = flatten_authed_error(err);

    assert!(!flat.contains("SESSION_EXPIRED"), "must not map: {flat}");
    assert!(flat.contains("connect timeout"), "cause preserved: {flat}");
    assert!(
        !crate::core::observability::is_session_expired_message(&flat),
        "non-auth error must NOT classify as session expiry: {flat}"
    );
}

#[test]
fn flatten_authed_error_does_not_swallow_message_not_found() {
    // `MessageNotFound` is a different expected state handled by its own callers
    // (channel streaming/delete paths downcast it); it must not be collapsed
    // into the session-expiry sentinel here.
    let err = anyhow::Error::new(BackendApiError::MessageNotFound {
        provider: "telegram".to_string(),
        message_id: "1103".to_string(),
    });
    let flat = flatten_authed_error(err);

    assert!(!flat.contains("SESSION_EXPIRED"), "must not map: {flat}");
    assert!(
        flat.contains("message not found"),
        "display preserved: {flat}"
    );
}

#[tokio::test]
async fn authed_json_403_is_not_demoted_to_unauthorized() {
    // 403 (Forbidden) is a genuine authorization/permission problem — the
    // token authenticated but lacked scope. That IS a code/config bug we
    // want to keep in Sentry; only 401 (token rejected as a whole) maps
    // to the expected-state `Unauthorized` variant.
    let app = Router::new().route(
        "/openai/v1/audio/speech",
        post(|| async { (axum::http::StatusCode::FORBIDDEN, "Forbidden") }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let base_url = format!("http://{addr}");
    let client = BackendClient::new(&base_url).unwrap();

    let err = client
        .authed_json("mock-jwt", Method::POST, "/openai/v1/audio/speech", None)
        .await
        .unwrap_err();
    assert!(
        err.downcast_ref::<BackendApiError>().is_none(),
        "403 must not be classified as Unauthorized"
    );
}

#[tokio::test]
async fn authed_json_404_outside_messages_path_still_reports() {
    // 404 on a non-`/channels/<provider>/messages/<id>` path should NOT be
    // demoted to MessageNotFound — it's a real backend bug or routing
    // mistake and must keep its Sentry signal.
    let app = Router::new().route(
        "/auth/profile",
        get(|| async { (axum::http::StatusCode::NOT_FOUND, "Not Found") }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let base_url = format!("http://{addr}");
    let client = BackendClient::new(&base_url).unwrap();

    let err = client
        .authed_json("mock-jwt", Method::GET, "/auth/profile", None)
        .await
        .unwrap_err();
    assert!(
        err.downcast_ref::<BackendApiError>().is_none(),
        "non-channel-message 404 must not be classified as MessageNotFound"
    );
}

#[tokio::test]
async fn sdk_backed_channel_typing_surfaces_unauthorized_on_401() {
    let app = Router::new().route(
        "/channels/telegram/typing",
        post(|| async { (axum::http::StatusCode::UNAUTHORIZED, "Unauthorized") }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = BackendClient::new(&format!("http://{addr}")).unwrap();
    let err = client
        .send_channel_typing("telegram", "mock-jwt")
        .await
        .unwrap_err();

    let typed = err.downcast_ref::<BackendApiError>().unwrap();
    let BackendApiError::Unauthorized { method, path } = typed else {
        panic!("expected Unauthorized, got {typed:?}");
    };
    assert_eq!(method, "POST");
    assert_eq!(path, "/channels/telegram/typing");
    // The session-expiry sentinel must still be derivable, so the dispatcher
    // keeps routing this to re-sign-in rather than to Sentry.
    assert!(flatten_authed_error(err).starts_with("SESSION_EXPIRED:"));
}

// ── typed channel-message 404s from the transport ───────────────────────────
//
// The transport (openhuman-tinyhumans) owns what a channel-message 404 means on
// the backend; these pin the recovery the core attaches to each variant.

/// A transport that answers every request with the error `make` builds.
struct FailingTransport(fn() -> crate::backend::BackendTransportError);

#[async_trait::async_trait]
impl crate::backend::BackendTransport for FailingTransport {
    async fn send_json(
        &self,
        _req: crate::backend::BackendRequest<'_>,
    ) -> Result<serde_json::Value, crate::backend::BackendTransportError> {
        Err((self.0)())
    }

    async fn send_multipart(
        &self,
        _req: crate::backend::BackendRequest<'_>,
        _form: reqwest::multipart::Form,
    ) -> Result<serde_json::Value, crate::backend::BackendTransportError> {
        Err((self.0)())
    }

    fn http_client(&self, _profile: crate::backend::TransportProfile) -> reqwest::Client {
        reqwest::Client::new()
    }

    fn base_url(
        &self,
        configured: Option<&str>,
        purpose: crate::backend::BaseUrlPurpose,
    ) -> String {
        crate::backend::transport::plain::PlainHttpTransport::new().base_url(configured, purpose)
    }

    fn product_identity(&self) -> String {
        crate::backend::transport::plain::TEST_PRODUCT_IDENTITY.to_string()
    }

    fn attribution_headers(&self) -> reqwest::header::HeaderMap {
        reqwest::header::HeaderMap::new()
    }

    fn name(&self) -> &'static str {
        "failing-test-transport"
    }
}

/// Run `f` with a [`FailingTransport`] installed. The `cfg(test)` global slot
/// is per thread and `#[tokio::test]` runs on one, so this cannot leak.
async fn with_failing_transport<F, Fut>(make: fn() -> crate::backend::BackendTransportError, f: F)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    crate::backend::install_backend_transport(std::sync::Arc::new(FailingTransport(make)));
    f().await;
    crate::backend::transport::install::clear_backend_transport();
}

#[tokio::test]
async fn route_missing_becomes_channel_edit_unsupported() {
    with_failing_transport(
        || crate::backend::BackendTransportError::ChannelMessageRouteMissing {
            provider: "telegram".into(),
            message_id: "1103".into(),
        },
        || async {
            let client = BackendClient::new("http://127.0.0.1:9").unwrap();
            let err = client
                .send_channel_edit("telegram", "1103", "mock-jwt", serde_json::json!({}))
                .await
                .unwrap_err();
            let typed = err.downcast_ref::<BackendApiError>().unwrap();
            let BackendApiError::ChannelEditUnsupported {
                provider,
                message_id,
            } = typed
            else {
                panic!("expected ChannelEditUnsupported, got {typed:?}");
            };
            assert_eq!(
                (provider.as_str(), message_id.as_str()),
                ("telegram", "1103")
            );
        },
    )
    .await;
}

#[tokio::test]
async fn message_not_found_becomes_message_not_found() {
    with_failing_transport(
        || crate::backend::BackendTransportError::ChannelMessageNotFound {
            provider: "discord".into(),
            message_id: "abc".into(),
        },
        || async {
            let client = BackendClient::new("http://127.0.0.1:9").unwrap();
            let err = client
                .send_channel_delete("discord", "abc", "mock-jwt")
                .await
                .unwrap_err();
            let typed = err.downcast_ref::<BackendApiError>().unwrap();
            let BackendApiError::MessageNotFound {
                provider,
                message_id,
            } = typed
            else {
                panic!("expected MessageNotFound, got {typed:?}");
            };
            assert_eq!((provider.as_str(), message_id.as_str()), ("discord", "abc"));
            // A missing message is an expected state: it keeps its own chain
            // rather than turning into a session-expiry sentinel.
            assert!(!flatten_authed_error(err).starts_with("SESSION_EXPIRED:"));
        },
    )
    .await;
}
