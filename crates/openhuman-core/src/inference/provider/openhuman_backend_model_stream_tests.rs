//! Managed-model SSE proxy replay and empty-response recovery.

use super::*;

async fn spawn_sse_chat_server(chunks: Vec<Value>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind SSE proxy");
    let addr = listener.local_addr().expect("SSE proxy address");
    let body = chunks
        .into_iter()
        .map(|chunk| format!("data: {chunk}\n\n"))
        .collect::<String>()
        + "data: [DONE]\n\n";
    let app = axum::Router::new()
        .route(
            "/openai/v1/chat/completions",
            axum::routing::post(
                |axum::extract::State(body): axum::extract::State<String>| async move {
                    (
                        [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                        body,
                    )
                },
            ),
        )
        .with_state(body);
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve SSE proxy");
    });
    addr.to_string()
}

async fn spawn_recovering_sse_proxy(
    first_delta: Value,
) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone)]
    struct ProxyState {
        first_delta: Value,
        calls: std::sync::Arc<AtomicUsize>,
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind recovering SSE proxy");
    let addr = listener.local_addr().expect("recovering proxy address");
    let calls = std::sync::Arc::new(AtomicUsize::new(0));
    let app = axum::Router::new()
        .route(
            "/openai/v1/chat/completions",
            axum::routing::post(
                |axum::extract::State(state): axum::extract::State<ProxyState>| async move {
                    let attempt = state.calls.fetch_add(1, Ordering::SeqCst);
                    let delta = if attempt == 0 {
                        state.first_delta
                    } else {
                        serde_json::json!({ "content": "Hello there" })
                    };
                    let body = format!(
                        "data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
                        serde_json::json!({ "choices": [{ "index": 0, "delta": delta }] }),
                        serde_json::json!({ "choices": [{ "index": 0, "delta": {}, "finish_reason": "stop" }] }),
                    );
                    ([(axum::http::header::CONTENT_TYPE, "text/event-stream")], body)
                },
            ),
        )
        .with_state(ProxyState {
            first_delta,
            calls: calls.clone(),
        });
    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("serve recovering SSE proxy");
    });
    (addr.to_string(), calls)
}

#[tokio::test]
async fn managed_stream_proxy_distinguishes_visible_and_reasoning_only_content() {
    use futures::StreamExt;
    use tinyinference_llm::model::StreamAccumulator;

    for (delta, expected_text) in [
        (
            serde_json::json!({ "content": "Hello there" }),
            "Hello there",
        ),
        (serde_json::json!({ "reasoning": "Hello there" }), ""),
        (serde_json::json!({ "content": "<think>Hello there" }), ""),
    ] {
        let tmp = tempfile::TempDir::new().expect("scratch credentials");
        seed_app_session(tmp.path());
        let addr = spawn_sse_chat_server(vec![
            serde_json::json!({ "choices": [{ "index": 0, "delta": delta }] }),
            serde_json::json!({ "choices": [{ "index": 0, "delta": {}, "finish_reason": "stop" }] }),
        ])
        .await;
        let backend = backend_pointed_at(&addr, tmp.path());
        let stream = backend
            .stream(&(), ModelRequest::new(vec![Message::user("hi")]))
            .await
            .expect("mock SSE response");
        let mut accumulated = StreamAccumulator::new();
        for item in stream.collect::<Vec<_>>().await {
            accumulated.push(&item);
        }
        let response = accumulated.finish().expect("completed SSE stream");
        assert_eq!(response.text(), expected_text);
        assert_eq!(response.finish_reason.as_deref(), Some("stop"));
    }
}

#[tokio::test]
async fn managed_stream_retries_a_proxy_completion_without_visible_text() {
    use std::sync::atomic::Ordering;
    use tinyagents_harness::context::RunConfig;
    use tinyagents_harness::runtime::{AgentHarness, RunPolicy};

    for first_delta in [
        serde_json::json!({ "reasoning": "Hello there" }),
        serde_json::json!({ "content": "<think>Hello there" }),
    ] {
        let tmp = tempfile::TempDir::new().expect("scratch credentials");
        seed_app_session(tmp.path());
        let (addr, calls) = spawn_recovering_sse_proxy(first_delta).await;
        let mut harness: AgentHarness<()> = AgentHarness::new();
        harness.register_model(
            "managed",
            std::sync::Arc::new(backend_pointed_at(&addr, tmp.path())),
        );
        harness.with_policy(RunPolicy {
            empty_response_retries: 1,
            ..RunPolicy::default()
        });

        let run = harness
            .invoke_streaming(
                &(),
                (),
                RunConfig::new("proxy-retry"),
                vec![Message::user("hi")],
            )
            .await
            .expect("second proxy completion should produce a visible answer");
        assert_eq!(run.text(), Some("Hello there".to_string()));
        assert_eq!(run.model_calls, 2);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
