use std::sync::Once;

use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use super::{authorize_dictation_request, DictationQuery};

fn test_token() -> String {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        crate::core_host::core::auth::init_rpc_token_with_value("dictation-http-tests-token")
            .expect("initialize test bearer");
    });
    crate::core_host::core::auth::get_rpc_token()
        .expect("test bearer initialized")
        .to_string()
}

fn query(token: Option<&str>) -> DictationQuery {
    DictationQuery {
        token: token.map(str::to_string),
    }
}

#[test]
fn dictation_rejects_disallowed_origin_before_authentication() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::ORIGIN,
        HeaderValue::from_static("https://attacker.example"),
    );

    let error = authorize_dictation_request(&headers, &query(Some(&test_token())))
        .expect_err("cross-origin request rejected");

    assert_eq!(error.status(), StatusCode::FORBIDDEN);
}

#[test]
fn dictation_rejects_missing_and_invalid_credentials() {
    for query in [query(None), query(Some("invalid"))] {
        let error = authorize_dictation_request(&HeaderMap::new(), &query)
            .expect_err("missing or invalid token rejected");
        assert_eq!(error.status(), StatusCode::UNAUTHORIZED);
    }
}

#[test]
fn dictation_accepts_bearer_header_and_browser_query_token() {
    let token = test_token();
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
    );
    assert!(authorize_dictation_request(&headers, &query(None)).is_ok());

    assert!(authorize_dictation_request(&HeaderMap::new(), &query(Some(&token))).is_ok());
}

#[tokio::test]
async fn dictation_handler_rejects_disallowed_origin_before_upgrade() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, crate::server::http::build_core_http_router(false))
            .await
            .unwrap();
    });

    let socket = tokio::net::TcpStream::connect(address).await.unwrap();
    let (read_half, mut write_half) = socket.into_split();
    let request = format!(
        "GET /ws/dictation HTTP/1.1\r\nHost: {address}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nAuthorization: Bearer {}\r\nOrigin: https://attacker.example\r\n\r\n",
        test_token()
    );
    write_half.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        BufReader::new(read_half).read_line(&mut response),
    )
    .await
    .expect("HTTP rejection completes")
    .unwrap();
    server.abort();

    assert!(response.starts_with("HTTP/1.1 403"), "{response}");
}
