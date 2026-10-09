//! Tests for the channel approval-surface and turn-state subscribers.

use super::*;
use crate::channels::traits::{ChannelMessage, SendMessage};
use std::path::Path;
use std::sync::Mutex as StdMutex;
use tempfile::tempdir;
use tinychannels::remote::session_store::with_store_read;

struct RecordingChannel {
    name: String,
    sent: StdMutex<Vec<SendMessage>>,
}

#[async_trait]
impl Channel for RecordingChannel {
    fn name(&self) -> &str {
        &self.name
    }
    async fn send(&self, message: &SendMessage) -> anyhow::Result<()> {
        self.sent.lock().unwrap().push(message.clone());
        Ok(())
    }
    async fn listen(&self, _tx: tokio::sync::mpsc::Sender<ChannelMessage>) -> anyhow::Result<()> {
        Ok(())
    }
}

fn recording(name: &str) -> Arc<RecordingChannel> {
    Arc::new(RecordingChannel {
        name: name.into(),
        sent: StdMutex::new(Vec::new()),
    })
}

fn approval_subscriber(channels: &[Arc<RecordingChannel>]) -> ChannelApprovalSurfaceSubscriber {
    let map: HashMap<String, Arc<dyn Channel>> = channels
        .iter()
        .map(|c| (c.name.clone(), Arc::clone(c) as Arc<dyn Channel>))
        .collect();
    ChannelApprovalSurfaceSubscriber::new(Arc::new(map))
}

fn received(channel: &str, reply_target: &str, thread_ts: Option<&str>, ws: &Path) -> DomainEvent {
    DomainEvent::ChannelMessageReceived {
        channel: channel.into(),
        message_id: "m1".into(),
        sender: "alice".into(),
        reply_target: reply_target.into(),
        content: "hi".into(),
        thread_ts: thread_ts.map(str::to_string),
        inbound_envelope: None,
        workspace_dir: ws.to_path_buf(),
    }
}

fn processed(channel: &str, reply_target: &str, ws: &Path) -> DomainEvent {
    DomainEvent::ChannelMessageProcessed {
        channel: channel.into(),
        message_id: "m1".into(),
        sender: "alice".into(),
        reply_target: reply_target.into(),
        content: "hi".into(),
        thread_ts: None,
        response: "ok".into(),
        provider: "test-provider".into(),
        model: "test-model".into(),
        elapsed_ms: 10,
        success: true,
        workspace_dir: ws.to_path_buf(),
    }
}

fn approval(thread_id: Option<&str>, client_id: Option<&str>) -> DomainEvent {
    DomainEvent::ApprovalRequested {
        request_id: "req-1".into(),
        tool_name: "file_write".into(),
        action_summary: "Write notes/today.md (1.2 KiB)".into(),
        args_redacted: serde_json::json!({"path": "notes/today.md"}),
        thread_id: thread_id.map(str::to_string),
        client_id: client_id.map(str::to_string),
        tool_call_id: None,
        expires_at: None,
        agent_id: None,
    }
}

#[tokio::test]
async fn approval_prompts_reach_every_chat_channel() {
    let ws = PathBuf::from("/tmp");
    for (channel, thread_ts, key) in [
        ("telegram", Some("7"), "telegram_alice_chat-1"),
        ("discord", None, "discord_alice_chat-1"),
        ("slack", Some("1700.1"), "slack_alice_chat-1_thread:1700.1"),
    ] {
        let ch = recording(channel);
        let sub = approval_subscriber(&[Arc::clone(&ch)]);
        assert_eq!(sub.name(), "channels::approval_surface");
        assert_eq!(sub.domains(), Some(&["channel", "approval"][..]));
        sub.handle(&received(channel, "chat-1", thread_ts, &ws))
            .await;
        assert!(sub.surface().reply_context(key).is_some(), "{channel}");
        sub.handle(&approval(Some(key), Some(channel))).await;
        let sent = ch.sent.lock().unwrap().clone();
        assert_eq!(sent.len(), 1, "{channel}");
        assert_eq!(sent[0].recipient, "chat-1");
        assert_eq!(sent[0].thread_ts.as_deref(), thread_ts);
        assert!(sent[0].content.contains("file_write"));
    }
}

#[tokio::test]
async fn approvals_for_web_email_or_unrouted_requests_are_ignored() {
    let ws = PathBuf::from("/tmp");
    let email = recording("email");
    let discord = recording("discord");
    let sub = approval_subscriber(&[Arc::clone(&email), Arc::clone(&discord)]);
    sub.handle(&received("email", "a@b.c", None, &ws)).await;
    sub.handle(&approval(Some("email_alice_a@b.c"), Some("email")))
        .await;
    sub.handle(&approval(Some("web-thread"), Some("web-client-1")))
        .await;
    sub.handle(&approval(Some("x"), None)).await;
    sub.handle(&approval(None, Some("discord"))).await;
    // No recorded context for this conversation → nothing sent.
    sub.handle(&approval(Some("discord_bob_c"), Some("discord")))
        .await;
    sub.handle(&DomainEvent::SystemStartup {
        component: "test".into(),
    })
    .await;
    assert!(email.sent.lock().unwrap().is_empty());
    assert!(discord.sent.lock().unwrap().is_empty());
}

fn busy(ws: &std::path::Path, channel: &str, target: &str) -> bool {
    with_store_read(ws, |store| Ok(store.is_busy(channel, target))).unwrap()
}

#[tokio::test]
async fn turn_state_tracks_busy_for_every_remote_control_channel() {
    let dir = tempdir().unwrap();
    let ws = dir.path().to_path_buf();
    let sub = ChannelTurnStateSubscriber::new(ws.clone());
    assert_eq!(sub.name(), "channels::turn_state");
    assert_eq!(sub.domains(), Some(&["channel"][..]));
    for channel in ["telegram", "discord"] {
        sub.handle(&received(channel, "chat-99", None, &ws)).await;
        assert!(busy(&ws, channel, "chat-99"), "{channel}");
        sub.handle(&processed(channel, "chat-99", &ws)).await;
        assert!(!busy(&ws, channel, "chat-99"), "{channel}");
    }
}

#[tokio::test]
async fn turn_state_ignores_non_remote_channels_and_stale_workspaces() {
    let dir = tempdir().unwrap();
    let ws = dir.path().to_path_buf();
    let sub = ChannelTurnStateSubscriber::new(ws.clone());
    sub.handle(&received("email", "a@b.c", None, &ws)).await;
    assert!(!busy(&ws, "email", "a@b.c"));
    let other = tempdir().unwrap().path().to_path_buf();
    sub.handle(&received("telegram", "chat-1", None, &other))
        .await;
    assert!(!busy(&ws, "telegram", "chat-1"));
}
