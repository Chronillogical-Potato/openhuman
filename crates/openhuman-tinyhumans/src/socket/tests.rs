use super::*;
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
