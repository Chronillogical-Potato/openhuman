use super::*;

fn get(path: &str) -> String {
    format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n")
}

#[test]
fn parses_normal_callback() {
    let (code, state) = parse_callback(&get("/auth/callback?code=abc123&state=xyz")).unwrap();
    assert_eq!(code, "abc123");
    assert_eq!(state, "xyz");
}

#[test]
fn decodes_percent_encoded_params() {
    let (code, state) = parse_callback(&get("/auth/callback?code=ab%2Bcd&state=x%3Dy")).unwrap();
    assert_eq!(code, "ab+cd");
    assert_eq!(state, "x=y");
}

#[test]
fn extra_params_are_ignored() {
    let (code, state) =
        parse_callback(&get("/auth/callback?code=c&state=s&session_state=ignored")).unwrap();
    assert_eq!(code, "c");
    assert_eq!(state, "s");
}

#[test]
fn missing_code_returns_error() {
    let err = parse_callback(&get("/auth/callback?state=s")).unwrap_err();
    assert!(err.to_string().contains("missing code"));
}

#[test]
fn missing_state_returns_error() {
    let err = parse_callback(&get("/auth/callback?code=c")).unwrap_err();
    assert!(err.to_string().contains("missing state"));
}

#[test]
fn non_callback_path_is_not_callback() {
    assert!(!is_callback_request(&get("/favicon.ico"), "/auth/callback"));
    assert!(!is_callback_request(&get("/"), "/auth/callback"));
}

#[test]
fn callback_path_with_code_is_callback() {
    assert!(is_callback_request(
        &get("/auth/callback?code=abc&state=xyz"),
        "/auth/callback"
    ));
}

#[test]
fn anthropic_callback_path_is_callback() {
    assert!(is_callback_request(
        &get("/callback?code=abc&state=xyz"),
        "/callback"
    ));
}

#[test]
fn auth_callback_path_does_not_match_anthropic_request() {
    // A request to /callback must not be accepted when callback_path is /auth/callback.
    assert!(!is_callback_request(
        &get("/callback?code=abc&state=xyz"),
        "/auth/callback"
    ));
}

#[tokio::test]
async fn bind_dynamic_port_returns_nonzero_port() {
    let server = bind(None).await.expect("bind should succeed");
    assert!(server.port > 0, "dynamic port should be nonzero");
}

#[tokio::test]
async fn bind_specific_port_returns_that_port() {
    let probe = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let free_port = probe.local_addr().unwrap().port();
    drop(probe);

    let server = bind(Some(free_port)).await.expect("bind should succeed");
    assert_eq!(server.port, free_port);
}
