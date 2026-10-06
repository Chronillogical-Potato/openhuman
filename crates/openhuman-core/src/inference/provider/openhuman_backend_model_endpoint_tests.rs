use super::*;

#[test]
fn api_key_endpoint_is_bound_to_tinyhumans_or_loopback() {
    use super::is_managed_endpoint_for_api_key;

    assert!(is_managed_endpoint_for_api_key(
        "https://api.tinyhumans.ai/openai/v1"
    ));
    assert!(is_managed_endpoint_for_api_key(
        "http://127.0.0.1:18765/openai/v1"
    ));
    assert!(is_managed_endpoint_for_api_key(
        "http://[::1]:18765/openai/v1"
    ));
    assert!(!is_managed_endpoint_for_api_key(
        "https://example.com/openai/v1"
    ));
    assert!(!is_managed_endpoint_for_api_key(
        "http://api.tinyhumans.ai/openai/v1"
    ));
}

/// #6724 (review): a 401 reported *inside* a stream must start re-auth just
/// like a failed `stream()` call does.
#[tokio::test]
async fn an_in_band_401_publishes_session_expired() {
    use crate::core::events::DomainEvent;

    crate::core::bus::init().await.expect("bus init");
    let mut rx = crate::core::bus::BUS
        .get()
        .expect("event bus initialized")
        .receiver();

    observe_in_band_failure(&ModelStreamItem::ProviderFailed(ProviderError {
        provider: PROVIDER_LABEL.to_string(),
        status: Some(401),
        message: "TEST_MARKER_IN_BAND token expired".to_string(),
        ..ProviderError::default()
    }));

    let mut source_seen = None;
    loop {
        match rx.try_recv() {
            Ok(DomainEvent::SessionExpired { source, reason })
                if reason.contains("TEST_MARKER_IN_BAND") =>
            {
                source_seen = Some(source);
                break;
            }
            Ok(_) | Err(tinybus::TryRecvError::Lagged(_)) => continue,
            Err(_) => break,
        }
    }
    assert_eq!(
        source_seen.as_deref(),
        Some("openhuman_backend_model.stream(401)"),
        "an in-band 401 must publish SessionExpired"
    );
}
