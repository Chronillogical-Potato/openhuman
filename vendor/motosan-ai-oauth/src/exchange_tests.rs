use super::*;
use crate::TokenBodyFormat;
use mockito::Matcher;

#[tokio::test]
async fn exchange_code_omits_state_for_random_strategy_form_body() {
    let mut server = mockito::Server::new_async().await;
    let token_url: &'static str = Box::leak(format!("{}/token", server.url()).into_boxed_str());

    let mock = server
        .mock("POST", "/token")
        .match_header(
            "content-type",
            Matcher::Regex("application/x-www-form-urlencoded".into()),
        )
        .match_body(Matcher::Exact(
            "grant_type=authorization_code&code=AUTHCODE&redirect_uri=http%3A%2F%2F127.0.0.1%2Fcb&code_verifier=VERIFIER&client_id=test-client"
                .into(),
        ))
        .with_status(200)
        .with_body(r#"{"access_token":"AT","refresh_token":"RT","expires_in":3600}"#)
        .create_async()
        .await;

    let cfg = crate::OAuthConfig {
        client_id: "test-client",
        client_secret: None,
        auth_url: "https://unused/",
        token_url,
        scopes: &[],
        redirect_port: None,
        callback_path: "/auth/callback",
        redirect_uri_host: "127.0.0.1",
        token_body: TokenBodyFormat::Form,
        extra_auth_params: &[],
        state_strategy: crate::StateStrategy::Random,
    };
    let token = exchange_code(&cfg, "AUTHCODE", "STATE", "VERIFIER", "http://127.0.0.1/cb")
        .await
        .expect("ok");
    assert_eq!(token.access_token, "AT");
    mock.assert_async().await;
}

#[tokio::test]
async fn exchange_code_omits_state_for_random_strategy_json_body() {
    let mut server = mockito::Server::new_async().await;
    let token_url: &'static str = Box::leak(format!("{}/token", server.url()).into_boxed_str());

    let mock = server
        .mock("POST", "/token")
        .match_header("content-type", Matcher::Regex("application/json".into()))
        // Matcher::JsonString compares parsed JSON, so key order in the
        // serialized HashMap does not matter.
        .match_body(Matcher::JsonString(
            r#"{"grant_type":"authorization_code","code":"AUTHCODE","redirect_uri":"http://127.0.0.1/cb","code_verifier":"VERIFIER","client_id":"test-client"}"#
            .into(),
        ))
        .with_status(200)
        .with_body(r#"{"access_token":"AT","refresh_token":"RT","expires_in":3600}"#)
        .create_async()
        .await;

    let cfg = crate::OAuthConfig {
        client_id: "test-client",
        client_secret: None,
        auth_url: "https://unused/",
        token_url,
        scopes: &[],
        redirect_port: None,
        callback_path: "/auth/callback",
        redirect_uri_host: "127.0.0.1",
        token_body: TokenBodyFormat::Json,
        extra_auth_params: &[],
        state_strategy: crate::StateStrategy::Random,
    };
    let token = exchange_code(&cfg, "AUTHCODE", "STATE", "VERIFIER", "http://127.0.0.1/cb")
        .await
        .expect("ok");
    assert_eq!(token.access_token, "AT");
    mock.assert_async().await;
}

#[tokio::test]
async fn exchange_code_sends_state_for_equals_verifier_strategy_json_body() {
    let mut server = mockito::Server::new_async().await;
    let token_url: &'static str = Box::leak(format!("{}/token", server.url()).into_boxed_str());

    let mock = server
        .mock("POST", "/token")
        .match_header("content-type", Matcher::Regex("application/json".into()))
        .match_body(Matcher::JsonString(
            r#"{"grant_type":"authorization_code","code":"AUTHCODE","state":"STATE","redirect_uri":"http://127.0.0.1/cb","code_verifier":"VERIFIER","client_id":"test-client"}"#
            .into(),
        ))
        .with_status(200)
        .with_body(r#"{"access_token":"AT","refresh_token":"RT","expires_in":3600}"#)
        .create_async()
        .await;

    let cfg = crate::OAuthConfig {
        client_id: "test-client",
        client_secret: None,
        auth_url: "https://unused/",
        token_url,
        scopes: &[],
        redirect_port: None,
        callback_path: "/auth/callback",
        redirect_uri_host: "127.0.0.1",
        token_body: TokenBodyFormat::Json,
        extra_auth_params: &[],
        state_strategy: crate::StateStrategy::EqualsVerifier,
    };
    let token = exchange_code(&cfg, "AUTHCODE", "STATE", "VERIFIER", "http://127.0.0.1/cb")
        .await
        .expect("ok");
    assert_eq!(token.access_token, "AT");
    mock.assert_async().await;
}
