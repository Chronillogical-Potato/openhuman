use super::*;

fn dummy_config() -> OAuthConfig {
    OAuthConfig {
        client_id: "test-client",
        client_secret: None,
        auth_url: "https://auth.example.com/oauth/authorize",
        token_url: "https://auth.example.com/oauth/token",
        scopes: &["openid", "profile"],
        redirect_port: None,
        callback_path: "/auth/callback",
        redirect_uri_host: "127.0.0.1",
        token_body: TokenBodyFormat::Form,
        extra_auth_params: &[],
        state_strategy: StateStrategy::Random,
    }
}

#[test]
fn build_auth_url_contains_required_params() {
    let config = dummy_config();
    let url = build_auth_url(
        &config,
        "challenge123",
        "state456",
        "http://127.0.0.1:9999/auth/callback",
    );
    assert!(url.contains("client_id=test-client"));
    assert!(url.contains("response_type=code"));
    assert!(url.contains("code_challenge=challenge123"));
    assert!(url.contains("code_challenge_method=S256"));
    assert!(url.contains("state=state456"));
    assert!(url.contains("redirect_uri="));
    assert!(url.contains("scope="));
}

#[test]
fn build_auth_url_includes_scopes_joined() {
    let config = dummy_config();
    let url = build_auth_url(&config, "c", "s", "http://127.0.0.1:1234/auth/callback");
    assert!(url.contains("scope=openid+profile") || url.contains("scope=openid%20profile"));
}

#[test]
fn build_auth_url_parses_as_valid_url() {
    let config = dummy_config();
    let url = build_auth_url(&config, "c", "s", "http://127.0.0.1:1234/auth/callback");
    reqwest::Url::parse(&url).expect("auth URL must be valid");
}

#[test]
fn build_auth_url_appends_extra_auth_params() {
    let config = OAuthConfig {
        extra_auth_params: &[("foo", "bar"), ("baz", "qux")],
        ..dummy_config()
    };
    let url = build_auth_url(&config, "c", "s", "http://127.0.0.1:1234/auth/callback");
    assert!(url.contains("foo=bar"));
    assert!(url.contains("baz=qux"));
}

#[test]
fn build_auth_url_no_longer_hardcodes_access_type() {
    // Empty extra_auth_params should produce no access_type query pair.
    let config = OAuthConfig {
        extra_auth_params: &[],
        ..dummy_config()
    };
    let url = build_auth_url(&config, "c", "s", "http://127.0.0.1:1234/auth/callback");
    assert!(!url.contains("access_type="));
}

#[test]
fn build_auth_url_uses_redirect_uri_host_in_uri() {
    // The redirect_uri is built by login(); we test the helper that constructs it.
    // The host substring comes from config.redirect_uri_host, not from the bind addr.
    let uri = build_redirect_uri("localhost", 53692, "/callback");
    assert_eq!(uri, "http://localhost:53692/callback");

    let uri = build_redirect_uri("127.0.0.1", 1455, "/auth/callback");
    assert_eq!(uri, "http://127.0.0.1:1455/auth/callback");
}

#[test]
fn token_is_expired_when_issued_at_zero() {
    let token = Token {
        access_token: "a".into(),
        refresh_token: "r".into(),
        id_token: None,
        expires_in: 3600,
        issued_at: 0,
    };
    assert!(token.is_expired());
}

#[test]
fn token_not_expired_when_just_issued() {
    let token = Token {
        access_token: "a".into(),
        refresh_token: "r".into(),
        id_token: None,
        expires_in: 3600,
        issued_at: unix_now(),
    };
    assert!(!token.is_expired());
}

#[test]
fn token_within_buffer_is_considered_expired() {
    let token = Token {
        access_token: "a".into(),
        refresh_token: "r".into(),
        id_token: None,
        expires_in: 30, // expires in 30s, within the 60s buffer
        issued_at: unix_now(),
    };
    assert!(token.is_expired());
}
