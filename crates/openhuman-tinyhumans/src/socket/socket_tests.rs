use super::*;
use openhuman_embed::__host::platform::socket::SocketPort;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[test]
fn sdk_token_callback_refreshes_each_attempt_and_stops_when_provider_fails() {
    let calls = Arc::new(AtomicUsize::new(0));
    let provider_calls = Arc::clone(&calls);
    let provider: openhuman_embed::__host::platform::socket::TokenProvider = Arc::new(move || {
        let call = provider_calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            if call < 2 {
                Ok(format!("token-{call}"))
            } else {
                Err("session disappeared".into())
            }
        })
    });
    let sdk_provider = sdk_token_provider(provider);
    assert_eq!(sdk_provider(), Some("token-0".to_string()));
    assert_eq!(sdk_provider(), Some("token-1".to_string()));
    assert_eq!(sdk_provider(), None);
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[test]
fn sdk_token_callback_rejects_empty_credentials() {
    let provider: openhuman_embed::__host::platform::socket::TokenProvider =
        Arc::new(|| Box::pin(async { Ok("  ".to_owned()) }));
    assert_eq!(sdk_token_provider(provider)(), None);
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

#[tokio::test]
async fn disconnected_port_rejects_emits_and_disconnect_is_idempotent() {
    let port = SdkSocketPort::default();
    assert!(port
        .emit("test:event", serde_json::json!({}))
        .await
        .is_err());
    assert!(port
        .emit_with_ack(
            "test:event",
            serde_json::json!({}),
            std::time::Duration::from_millis(1),
        )
        .await
        .is_err());
    port.disconnect().await.unwrap();
    assert!(!port.is_loop_active());
    assert!(!port.loop_stopped_on_failure());
}

#[tokio::test]
async fn actor_shutdown_fails_an_emit_queued_during_reconnect_promptly() {
    use tokio::sync::{mpsc, oneshot, watch};
    use tinyhumans_sdk::TinyHumansClient;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let connection = TinyHumansClient::new(format!("http://{address}"))
        .connect_socket_with_token_provider(|| Some("token".to_owned()))
        .await
        .unwrap();

    let (commands, receiver) = mpsc::channel(4);
    let (shutdown, shutdown_receiver) = watch::channel(false);
    let (done, done_receiver) = oneshot::channel();
    let state = Arc::new(RwLock::new(SocketState::default()));
    let loop_active = Arc::new(AtomicBool::new(true));
    let stopped_on_failure = Arc::new(AtomicBool::new(false));
    tokio::spawn(run_socket_actor(
        connection,
        receiver,
        shutdown_receiver,
        done,
        state,
        Arc::clone(&loop_active),
        stopped_on_failure,
    ));

    let (result, response) = oneshot::channel();
    commands
        .send(SocketCommand::Emit {
            event: "test:event".to_owned(),
            data: serde_json::json!({}),
            result,
        })
        .await
        .unwrap();
    tokio::task::yield_now().await;
    shutdown.send_replace(true);

    assert!(tokio::time::timeout(std::time::Duration::from_millis(250), response)
        .await
        .expect("the queued emit waiter must be released during shutdown")
        .unwrap()
        .is_err());
    tokio::time::timeout(std::time::Duration::from_millis(250), done_receiver)
        .await
        .expect("the actor must stop during shutdown")
        .unwrap();
    assert!(!loop_active.load(Ordering::Acquire));
}
