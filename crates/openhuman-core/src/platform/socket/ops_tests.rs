use std::sync::atomic::{AtomicBool, Ordering};

use super::*;

#[tokio::test]
async fn static_token_connection_clears_identity_state_after_disconnect() {
    let manager = SocketManager::new();
    manager
        .connect("http://127.0.0.1:1", "opaque-token")
        .await
        .unwrap();
    let cleared = AtomicBool::new(false);
    connect_static_using(&manager, "http://127.0.0.1:1", "replacement", || {
        assert_eq!(
            manager.get_state().status,
            crate::platform::socket::types::ConnectionStatus::Disconnected
        );
        cleared.store(true, Ordering::SeqCst);
    })
    .await
    .unwrap();
    assert!(cleared.load(Ordering::SeqCst));
}

// ── Redundant-connect suppression (#6181) ──────────────────────────
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use crate::platform::socket::token_provider::{static_token_provider, TokenProvider};
use crate::platform::socket::types::ConnectionStatus;

const TOKEN_A: &str = "session-token-a";
const TOKEN_B: &str = "session-token-b";

/// EIO v4 mock that counts accepts and completes enough of the handshake for
/// `SocketManager` to reach `Connected`, then idles. Accepting in a loop is the
/// point: a duplicate connect shows up as a second accept.
async fn spawn_accept_counting_eio_server() -> (Arc<AtomicUsize>, std::net::SocketAddr) {
    use futures_util::{SinkExt, StreamExt};
    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, tungstenite::Message as WsMessage};

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let accepts = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&accepts);
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            counter.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move {
                let Ok(ws) = accept_async(stream).await else {
                    return;
                };
                let (mut write, mut read) = ws.split();
                // Long ping intervals so nothing reconnects on its own mid-test.
                let open = r#"0{"sid":"mock-eio-sid","upgrades":[],"pingInterval":30000,"pingTimeout":30000}"#;
                let _ = write.send(WsMessage::Text(open.into())).await;
                let _ = read.next().await; // client SIO CONNECT
                let _ = write
                    .send(WsMessage::Text(r#"40{"sid":"mock-sio-sid"}"#.into()))
                    .await;
                while let Some(Ok(_)) = read.next().await {}
            });
        }
    });
    (accepts, addr)
}

/// EIO v4 mock that completes one handshake and then holds the connection open
/// until the returned sender says to hang up, so a test can decide exactly when
/// the loop is forced to reconnect. Later connections are held indefinitely.
async fn spawn_server_that_hangs_up_on_cue(
) -> (std::net::SocketAddr, tokio::sync::watch::Sender<bool>) {
    use futures_util::{SinkExt, StreamExt};
    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, tungstenite::Message as WsMessage};

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let (hang_up, rx) = tokio::sync::watch::channel(false);
    tokio::spawn(async move {
        let mut first = true;
        while let Ok((stream, _)) = listener.accept().await {
            let mut rx = rx.clone();
            let hang_up_this_one = std::mem::take(&mut first);
            tokio::spawn(async move {
                let Ok(ws) = accept_async(stream).await else {
                    return;
                };
                let (mut write, mut read) = ws.split();
                let open = r#"0{"sid":"mock-eio-sid","upgrades":[],"pingInterval":30000,"pingTimeout":30000}"#;
                let _ = write.send(WsMessage::Text(open.into())).await;
                let _ = read.next().await; // client SIO CONNECT
                let _ = write
                    .send(WsMessage::Text(r#"40{"sid":"mock-sio-sid"}"#.into()))
                    .await;
                if hang_up_this_one {
                    while rx.changed().await.is_ok() {
                        if *rx.borrow() {
                            break;
                        }
                    }
                    let _ = write.close().await;
                    return;
                }
                while let Some(Ok(_)) = read.next().await {}
            });
        }
    });
    (addr, hang_up)
}

async fn wait_for_connected(manager: &SocketManager) {
    for _ in 0..200 {
        if manager.is_connected() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    panic!("socket never reached Connected");
}

/// Wait out the window in which a duplicate connect would reach the server.
///
/// `connect_with_provider` returns as soon as the background loop is spawned, so
/// reading the accept counter straight afterwards would pass even when a second
/// session is on its way — a vacuous assertion. Returns early the moment a
/// second accept does show up, so the fix's happy path is the only one that pays
/// the full wait.
async fn settle_for_a_second_accept(accepts: &AtomicUsize) {
    for _ in 0..40 {
        if accepts.load(Ordering::SeqCst) > 1 {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
}

/// Stand-in for the core's bootstrap auto-connect (`spawn_socket_auto_connect`).
async fn bootstrap_auto_connect(manager: &SocketManager, url: &str, token: &str) {
    manager
        .connect_with_provider(url, static_token_provider(token.to_string()))
        .await
        .unwrap();
    wait_for_connected(manager).await;
}

/// #6181: on a cold start the core auto-connects and the renderer's
/// `socket_connect_with_session` RPC follows a couple of seconds later with the
/// same backend URL and the same stored token. The second one must reuse the
/// live socket rather than tear it down and open a fresh EIO session.
#[tokio::test]
async fn a_redundant_session_connect_reuses_the_live_socket() {
    let (accepts, addr) = spawn_accept_counting_eio_server().await;
    let url = format!("http://{addr}");
    let manager = SocketManager::new();

    bootstrap_auto_connect(&manager, &url, TOKEN_A).await;

    let bridge_installed = AtomicBool::new(false);
    let state = connect_with_session_using(
        &manager,
        &url,
        TOKEN_A,
        static_token_provider(TOKEN_A.to_string()),
        || bridge_installed.store(true, Ordering::SeqCst),
    )
    .await
    .unwrap();

    settle_for_a_second_accept(&accepts).await;
    assert_eq!(
        accepts.load(Ordering::SeqCst),
        1,
        "a redundant connect for an identical identity opened a duplicate EIO session"
    );
    // The caller gets the live socket's state back, not the `Connecting` of a
    // handshake that has only just been kicked off.
    assert_eq!(state.status, ConnectionStatus::Connected);
    // Reusing the socket must not skip the bridge: it is pinned to a `Config`,
    // and `connect_static` can have cleared it while leaving a matching identity
    // behind, so the workflow plane would be left stale or disabled.
    assert!(
        bridge_installed.load(Ordering::SeqCst),
        "reusing the socket skipped the workflow-bridge install"
    );
}

/// The bridge half of the reuse path, end to end through the operation that
/// clears it: `openhuman.socket_connect` disables the identity-bound workflow
/// plane and leaves a matching connection identity behind, so a following
/// `connect_with_session` for the same url+token must still restore it.
#[tokio::test]
async fn a_reused_socket_still_restores_a_bridge_a_static_connect_cleared() {
    let (accepts, addr) = spawn_accept_counting_eio_server().await;
    let url = format!("http://{addr}");
    let manager = SocketManager::new();

    let cleared = AtomicBool::new(false);
    connect_static_using(&manager, &url, TOKEN_A, || {
        cleared.store(true, Ordering::SeqCst)
    })
    .await
    .unwrap();
    wait_for_connected(&manager).await;
    assert!(cleared.load(Ordering::SeqCst));

    let bridge_installed = AtomicBool::new(false);
    connect_with_session_using(
        &manager,
        &url,
        TOKEN_A,
        static_token_provider(TOKEN_A.to_string()),
        || bridge_installed.store(true, Ordering::SeqCst),
    )
    .await
    .unwrap();

    settle_for_a_second_accept(&accepts).await;
    assert!(
        bridge_installed.load(Ordering::SeqCst),
        "the workflow plane stayed disabled after a static connect cleared it"
    );
    assert_eq!(
        accepts.load(Ordering::SeqCst),
        1,
        "restoring the bridge should not cost a fresh EIO session"
    );
}

/// `ws_loop` re-reads the token provider before every attempt, so a session
/// refreshed mid-loop leaves the socket authenticated with a token the manager
/// was never handed. The recorded identity has to follow, or the next session
/// connect tears down a perfectly healthy socket.
#[tokio::test]
async fn a_token_refreshed_mid_loop_updates_the_recorded_identity() {
    let (addr, hang_up) = spawn_server_that_hangs_up_on_cue().await;
    let url = format!("http://{addr}");
    let manager = SocketManager::new();

    // A provider whose answer changes under the loop, exactly as a live session
    // refresh does.
    let refreshed = Arc::new(AtomicBool::new(false));
    let seen = Arc::clone(&refreshed);
    let provider: TokenProvider = Arc::new(move || {
        Ok(if seen.load(Ordering::SeqCst) {
            TOKEN_B.to_string()
        } else {
            TOKEN_A.to_string()
        })
    });

    manager.connect_with_provider(&url, provider).await.unwrap();
    wait_for_connected(&manager).await;
    assert!(manager.is_live_for(&url, TOKEN_A));

    // Refresh the session, then drop the socket so the loop reconnects with the
    // new token.
    refreshed.store(true, Ordering::SeqCst);
    let _ = hang_up.send(true);

    for _ in 0..200 {
        if manager.is_live_for(&url, TOKEN_B) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("the recorded identity still names the pre-refresh token after a reconnect");
}

/// The other half of the guard: an account switch keeps the same backend URL but
/// arrives with a different token, and must still disconnect, rebind the
/// identity-bound workflow plane, and reconnect.
#[tokio::test]
async fn a_session_connect_with_a_different_token_still_rebinds() {
    let (accepts, addr) = spawn_accept_counting_eio_server().await;
    let url = format!("http://{addr}");
    let manager = SocketManager::new();

    bootstrap_auto_connect(&manager, &url, TOKEN_A).await;

    let bridge_installed = AtomicBool::new(false);
    connect_with_session_using(
        &manager,
        &url,
        TOKEN_B,
        static_token_provider(TOKEN_B.to_string()),
        || bridge_installed.store(true, Ordering::SeqCst),
    )
    .await
    .unwrap();
    wait_for_connected(&manager).await;

    assert!(
        bridge_installed.load(Ordering::SeqCst),
        "a new session token must still rebind the identity-bound workflow plane"
    );
    assert_eq!(
        accepts.load(Ordering::SeqCst),
        2,
        "a new session token must open a fresh EIO session"
    );
}

/// `is_live_for` is identity-scoped, not merely "am I connected": a different
/// backend URL is a different identity, and a disconnect clears the record so a
/// later connect is never skipped against a dead socket.
#[tokio::test]
async fn is_live_for_is_scoped_to_url_and_token_and_cleared_on_disconnect() {
    let (_accepts, addr) = spawn_accept_counting_eio_server().await;
    let url = format!("http://{addr}");
    let manager = SocketManager::new();

    bootstrap_auto_connect(&manager, &url, TOKEN_A).await;

    assert!(manager.is_live_for(&url, TOKEN_A));
    assert!(!manager.is_live_for(&url, TOKEN_B));
    assert!(!manager.is_live_for("http://127.0.0.1:1", TOKEN_A));

    manager.disconnect().await.unwrap();
    assert!(!manager.is_live_for(&url, TOKEN_A));
}

// ── Mid-handshake redundant-connect suppression (#6418) ────────────

/// EIO v4 mock that accepts and counts connections but holds each one *before*
/// the Engine.IO OPEN frame until the returned gate is opened.
///
/// The ungated mock races: `connect_with_provider` returns at spawn, so a test
/// that fired the second connect straight afterwards would sometimes find the
/// socket already `Connected` and exercise the #6181 path instead of this one.
/// Stalling the handshake pins the manager in `Connecting` for as long as the
/// test needs, which is the exact window #6418 lives in.
async fn spawn_handshake_stalling_eio_server() -> (
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
    std::net::SocketAddr,
    tokio::sync::watch::Sender<bool>,
) {
    use futures_util::{SinkExt, StreamExt};
    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, tungstenite::Message as WsMessage};

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let accepts = Arc::new(AtomicUsize::new(0));
    let sio_acks = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&accepts);
    let ack_counter = Arc::clone(&sio_acks);
    let (gate, rx) = tokio::sync::watch::channel(false);
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            counter.fetch_add(1, Ordering::SeqCst);
            let mut rx = rx.clone();
            let ack_counter = Arc::clone(&ack_counter);
            tokio::spawn(async move {
                let Ok(ws) = accept_async(stream).await else {
                    return;
                };
                let (mut write, mut read) = ws.split();
                // Hold here: accepted, but no EIO OPEN yet, so the client stays
                // in `Connecting`.
                while !*rx.borrow() {
                    if rx.changed().await.is_err() {
                        return;
                    }
                }
                let open = r#"0{"sid":"mock-eio-sid","upgrades":[],"pingInterval":30000,"pingTimeout":30000}"#;
                let _ = write.send(WsMessage::Text(open.into())).await;
                let _ = read.next().await; // client SIO CONNECT
                                           // The literal artefact Alan's acceptance criterion names: one
                                           // `SIO CONNECT ACK` per boot. Counted at the send, so it tracks
                                           // completed Socket.IO handshakes, not merely TCP accepts.
                let _ = write
                    .send(WsMessage::Text(r#"40{"sid":"mock-sio-sid"}"#.into()))
                    .await;
                ack_counter.fetch_add(1, Ordering::SeqCst);
                while let Some(Ok(_)) = read.next().await {}
            });
        }
    });
    (accepts, sio_acks, addr, gate)
}

/// Wait until the manager is mid-handshake: the server has accepted a socket
/// and the manager is in `Connecting`. Panics rather than letting a test assert
/// against a window that never opened.
async fn wait_for_mid_handshake(manager: &SocketManager, accepts: &AtomicUsize) {
    for _ in 0..200 {
        if accepts.load(Ordering::SeqCst) >= 1
            && manager.get_state().status == ConnectionStatus::Connecting
        {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    panic!("the socket never sat in Connecting with a connection accepted");
}

/// #6418: `spawn_loop` returns when the background task is spawned, not when the
/// handshake completes, so `lock_identity_rebind` is released while the first
/// path is still mid-handshake. The renderer's RPC then arrives, and before this
/// fix saw `Connecting` — not `Connected` — decided the socket was not live for
/// its identity, and called `disconnect()` on a handshake that was seconds from
/// done. Alan's boot log is exactly that: SIO CONNECT ACK, "Shutdown signal
/// received", then a second "Connecting to".
#[tokio::test]
async fn a_session_connect_during_the_boot_handshake_does_not_restart_it() {
    let (accepts, sio_acks, addr, gate) = spawn_handshake_stalling_eio_server().await;
    let url = format!("http://{addr}");
    let manager = SocketManager::new();

    // Bootstrap auto-connect, deliberately NOT waiting for `Connected`.
    manager
        .connect_with_provider(&url, static_token_provider(TOKEN_A.to_string()))
        .await
        .unwrap();
    wait_for_mid_handshake(&manager, &accepts).await;

    // The renderer's RPC lands in that window with the same url+token.
    let bridge_installed = AtomicBool::new(false);
    connect_with_session_using(
        &manager,
        &url,
        TOKEN_A,
        static_token_provider(TOKEN_A.to_string()),
        || bridge_installed.store(true, Ordering::SeqCst),
    )
    .await
    .unwrap();

    // Let the stalled handshake finish and settle past the duplicate window.
    let _ = gate.send(true);
    wait_for_connected(&manager).await;
    settle_for_a_second_accept(&accepts).await;

    assert_eq!(
        sio_acks.load(Ordering::SeqCst),
        1,
        "boot produced more than one SIO CONNECT ACK: a connect arriving mid-handshake tore the boot handshake down and redid it"
    );
    assert_eq!(
        accepts.load(Ordering::SeqCst),
        1,
        "a connect arriving mid-handshake opened a second EIO session"
    );
    // The socket is reused, but the bridge is pinned to a `Config` and must
    // still be installed on the reuse path.
    assert!(
        bridge_installed.load(Ordering::SeqCst),
        "reusing an in-flight handshake skipped the workflow-bridge install"
    );
}

/// The guard widened to `Connecting`, so it must not be satisfied by a stale
/// `Connecting` with no loop behind it — otherwise a manager that never got a
/// task spawned would answer "already connecting" forever and never reconnect.
#[tokio::test]
async fn a_connecting_status_with_no_live_loop_is_not_treated_as_serving() {
    let manager = SocketManager::new();
    *manager.shared.connection_identity.write() =
        Some(("http://127.0.0.1:1".to_string(), TOKEN_A.to_string()));
    *manager.shared.status.write() = ConnectionStatus::Connecting;

    assert!(
        !manager.is_live_for("http://127.0.0.1:1", TOKEN_A),
        "a Connecting status with no background loop was treated as a live connection"
    );
}
