use super::*;
use openhuman_embed::__host::platform::socket::SocketPort;
use std::sync::{atomic::{AtomicUsize, Ordering}, Arc};

#[test]
fn sdk_token_callback_refreshes_each_attempt_and_stops_when_provider_fails() {
    let calls = Arc::new(AtomicUsize::new(0));
    let provider_calls = Arc::clone(&calls);
    let provider: openhuman_embed::__host::platform::socket::TokenProvider = Arc::new(move || {
        let call = provider_calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            if call < 2 { Ok(format!("token-{call}")) }
            else { Err("session disappeared".into()) }
        })
    });
    let mut sdk_provider = sdk_token_provider(provider);
    assert_eq!(sdk_provider(), Some("token-0".to_string()));
    assert_eq!(sdk_provider(), Some("token-1".to_string()));
    assert_eq!(sdk_provider(), None);
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[test]
fn socket_identity_match_requires_both_url_and_session_token() {
    use openhuman_embed::__host::platform::socket::{ConnectionStatus, SocketState};

    let port = SdkSocketPort::default();
    *port.identity.write() = Some(("https://api.example".into(), "session-a".into()));
    *port.state.write() = SocketState {
        status: ConnectionStatus::Connected,
        socket_id: None,
        error: None,
    };
    assert!(port.is_live_for("https://api.example", "session-a"));
    assert!(!port.is_live_for("https://api.example", "session-b"));
    assert!(!port.is_live_for("https://other.example", "session-a"));
}
