use super::*;

#[test]
fn local_offline_token_never_constructs_a_hosted_integration_client() {
    let config = crate::config::Config::default();
    let backend_url = "https://api.example.test".to_owned();
    assert!(build_client_with_session_token(
        &config,
        backend_url.clone(),
        Some("header.payload.local".into()),
    )
    .is_none());
    assert!(build_client_with_session_token(
        &config,
        backend_url,
        Some("header.payload.jwt".into()),
    )
    .is_some());
}
