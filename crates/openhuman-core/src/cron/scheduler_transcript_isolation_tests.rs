//! A cron agent turn must not resume an unrelated transcript.
//!
//! A cron agent has no thread, so without an override its first turn falls
//! back to `ResumeMode::LatestForAgent` and replays the newest unthreaded
//! transcript on disk for the agent name — some old conversation, with its
//! frozen system prompt and tool names. In production that handed the
//! `apple_stock_daily_email` job a Sep 20 orchestrator session whose prompt
//! still advertised the removed `delegate_to_integrations_agent`, so the run
//! could not send its email.
//!
//! Both tests drive the real turn against a local OpenAI-compatible endpoint
//! that records every request body, and each carries its own control: the
//! control proves the stale transcript IS replayed when the override is
//! absent, so the fixed-path assertion is not passing vacuously.

use super::*;
use crate::config::schema::cloud_providers::{AuthStyle, CloudProviderCreds};
use crate::cron::JobType;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Marker planted only in the stale transcript. It reaches a provider request
/// if and only if that transcript was resumed.
const STALE_MARKER: &str = "STALE-SEP20-CONVERSATION-MARKER";
const JOB_PROMPT: &str = "Look up the AAPL price and report it.";

/// Minimal OpenAI-compatible endpoint: records each request body and answers
/// every completion with one final text message (streamed or not, matching
/// the request).
async fn spawn_recording_provider() -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let bodies = Arc::new(Mutex::new(Vec::new()));
    let recorded = bodies.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let recorded = recorded.clone();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 8192];
                let (head_end, content_length) = loop {
                    let n = match socket.read(&mut chunk).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => n,
                    };
                    buf.extend_from_slice(&chunk[..n]);
                    if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&buf[..pos]).to_ascii_lowercase();
                        let len = head
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length:"))
                            .and_then(|v| v.trim().parse::<usize>().ok())
                            .unwrap_or(0);
                        break (pos + 4, len);
                    }
                };
                while buf.len() < head_end + content_length {
                    match socket.read(&mut chunk).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => buf.extend_from_slice(&chunk[..n]),
                    }
                }
                let body = String::from_utf8_lossy(&buf[head_end..]).to_string();
                eprintln!("DBGREQ {} :: {}", String::from_utf8_lossy(&buf[..head_end]).lines().next().unwrap_or(""), &body.chars().take(300).collect::<String>());
                let streaming = body.contains("\"stream\":true");
                recorded.lock().unwrap().push(body);

                let (content_type, payload) = if streaming {
                    let delta = serde_json::json!({
                        "id": "c1", "object": "chat.completion.chunk", "created": 0,
                        "model": "local-model",
                        "choices": [{"index": 0, "delta": {"role": "assistant", "content": "AAPL is $337.02."}, "finish_reason": null}]
                    });
                    let stop = serde_json::json!({
                        "id": "c1", "object": "chat.completion.chunk", "created": 0,
                        "model": "local-model",
                        "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
                        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
                    });
                    (
                        "text/event-stream",
                        format!("data: {delta}\n\ndata: {stop}\n\ndata: [DONE]\n\n"),
                    )
                } else {
                    (
                        "application/json",
                        serde_json::json!({
                            "id": "c1", "object": "chat.completion", "created": 0,
                            "model": "local-model",
                            "choices": [{"index": 0, "message": {"role": "assistant", "content": "AAPL is $337.02."}, "finish_reason": "stop"}],
                            "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
                        })
                        .to_string(),
                    )
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{payload}",
                    payload.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });
    (format!("http://{addr}"), bodies)
}

async fn config_with_provider(tmp: &TempDir, endpoint: &str) -> Config {
    let ws = tmp.path().join("workspace");
    tokio::fs::create_dir_all(&ws).await.unwrap();
    let mut config = Config {
        workspace_dir: ws.clone(),
        action_dir: ws,
        config_path: tmp.path().join("config.toml"),
        ..Config::default()
    };
    config.cloud_providers = vec![CloudProviderCreds {
        id: "local-recorder".into(),
        slug: "lmstudio".into(),
        label: "Recorder".into(),
        endpoint: endpoint.into(),
        auth_style: AuthStyle::None,
        ..Default::default()
    }];
    config.default_model = Some("lmstudio:local-model".into());
    config.chat_provider = Some("lmstudio:local-model".into());
    config
}

/// Plant an unthreaded orchestrator transcript shaped like the stale one found
/// in production (`<unix>_orchestrator.jsonl`, no `session_id`).
fn seed_stale_orchestrator_transcript(config: &Config) {
    let dir = config.workspace_dir.join("session_raw");
    std::fs::create_dir_all(&dir).unwrap();
    let lines = [
        serde_json::json!({"_meta": {
            "version": 1, "agent": "orchestrator", "agent_id": "orchestrator",
            "agent_type": "root", "dispatcher": "native", "model": "chat-v1",
            "created": "2026-09-20T15:34:17.599174+00:00",
            "updated": "2026-09-20T15:34:24.782492+00:00",
            "turn_count": 1, "input_tokens": 0, "output_tokens": 0,
            "cached_input_tokens": 0, "charged_amount_usd": 0.0,
            "thread_id": "thread-a4dd77d5-7b00-424b-a2b9-da41d8bc343a"
        }}),
        serde_json::json!({"role": "system", "content": format!("Old frozen prompt. {STALE_MARKER} Use delegate_to_integrations_agent for email.")}),
        serde_json::json!({"role": "user", "content": format!("summarize my inbox {STALE_MARKER}")}),
        serde_json::json!({"role": "assistant", "content": "your inbox, latest 6: ..."}),
    ];
    let body = lines
        .iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(dir.join("1790201093_orchestrator.jsonl"), body + "\n").unwrap();
}

fn agent_job() -> CronJob {
    let mut job = test_job("");
    job.id = "e6ab35f2-64e2-4d23-a885-88f48ee2919e".into();
    job.name = Some("apple_stock_daily_email".into());
    job.job_type = JobType::Agent;
    job.prompt = Some(JOB_PROMPT.into());
    job
}

fn requests_with_job_prompt(bodies: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    bodies
        .lock()
        .unwrap()
        .iter()
        .filter(|body| body.contains(JOB_PROMPT))
        .cloned()
        .collect()
}

#[tokio::test]
async fn cron_agent_turn_does_not_resume_a_stale_unthreaded_transcript() {
    crate::agent::harness::definition::AgentDefinitionRegistry::init_global_builtins()
        .expect("init built-in agent definitions");

    // CONTROL — the same agent, same workspace, same stale transcript, run
    // without the override: the stale conversation reaches the provider. If
    // this ever stops holding, the assertion below proves nothing.
    {
        let (endpoint, bodies) = spawn_recording_provider().await;
        let tmp = TempDir::new().unwrap();
        let config = config_with_provider(&tmp, &endpoint).await;
        seed_stale_orchestrator_transcript(&config);
        let job = agent_job();
        let BuiltCronAgent { mut agent } =
            build_agent_for_cron_job(&config, &job).expect("build cron agent");
        agent.set_event_context(format!("cron:{}", job.id), "cron");
        agent
            .run_single(&format!("[cron:{} x] {JOB_PROMPT}", job.id))
            .await
            .expect("control turn");
        let turn_requests = requests_with_job_prompt(&bodies);
        assert!(
            !turn_requests.is_empty(),
            "control: the job turn must reach the recording provider"
        );
        assert!(
            turn_requests.iter().any(|body| body.contains(STALE_MARKER)),
            "control: without the override an unthreaded cron turn must replay the \
             stale transcript, otherwise this test cannot prove the fix does anything"
        );
    }

    // FIXED — the production entry point.
    let (endpoint, bodies) = spawn_recording_provider().await;
    let tmp = TempDir::new().unwrap();
    let config = config_with_provider(&tmp, &endpoint).await;
    seed_stale_orchestrator_transcript(&config);
    let (success, output, raw_error) = run_agent_job(&config, &agent_job()).await;
    assert!(success, "cron agent job should succeed: {output} / {raw_error:?}");
    let turn_requests = requests_with_job_prompt(&bodies);
    assert!(
        !turn_requests.is_empty(),
        "the job turn must reach the recording provider"
    );
    for body in &turn_requests {
        assert!(
            !body.contains(STALE_MARKER),
            "a cron turn must start from a fresh prompt, not the stale transcript"
        );
        assert!(
            !body.contains("delegate_to_integrations_agent"),
            "the stale frozen prompt's removed tool must not reach the model"
        );
    }
}
