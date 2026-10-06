//! Regression for #920: the embedded JSON-RPC server's `axum::serve` accept
//! loop must stop within the cancellation timeout when its `CancellationToken`
//! fires.
//!
//! This used to be a `#[ignore]`d unit test in `openhuman-rpc`
//! (`shims_tests.rs`, #1552). `run_server_embedded` runs the full production
//! bootstrap, which spawns background tasks and writes process-global statics
//! (`scheduler_gate::STATE`, `SIGNED_OUT`, the LLM permit semaphore, the
//! `agent.run_turn` registry, ...) with no teardown, so inside the shared unit
//! test binary it raced with every sibling test. A root `tests/` target is its
//! own process, which is exactly the isolation the old ignore reason asked for.

use std::time::Duration;

use tokio_util::sync::CancellationToken;

async fn wait_until_port(port: u16, accepting: bool) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let connected = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok();
        if connected == accepting {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "port {port} never became accepting={accepting}"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_token_stops_axum_listener_within_timeout() {
    openhuman_core::cron::scheduler_gate::set_signed_out(false);

    let workspace = tempfile::tempdir().expect("workspace tempdir");
    // Pin the scheduler-gate policy so the bootstrap's `init_global` snapshot
    // cannot freeze the cached policy at Paused under CPU pressure.
    std::fs::write(
        workspace.path().join("config.toml"),
        "[scheduler_gate]\nmode = \"always_on\"\n",
    )
    .expect("seed scheduler_gate=always_on config.toml");
    // This target runs one test in its own process; nothing else reads these.
    std::env::set_var("OPENHUMAN_WORKSPACE", workspace.path());
    std::env::set_var("OPENHUMAN_DISABLE_CHANNEL_LISTENERS", "1");
    std::env::set_var("OPENHUMAN_CORE_TOKEN", "test-token-shutdown");

    let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("allocate test port");
    let port = probe.local_addr().expect("local addr").port();
    drop(probe);

    let shutdown_token = CancellationToken::new();
    let server_token = shutdown_token.clone();
    let server = tokio::spawn(async move {
        openhuman_rpc::server::run_server_embedded(
            Some("127.0.0.1"),
            Some(port),
            false,
            server_token,
        )
        .await
    });

    wait_until_port(port, true).await;
    shutdown_token.cancel();

    let result = tokio::time::timeout(Duration::from_secs(10), server)
        .await
        .expect("embedded server task should stop within timeout")
        .expect("embedded server task should not panic");
    result.expect("embedded server should shut down cleanly");
    wait_until_port(port, false).await;
}
