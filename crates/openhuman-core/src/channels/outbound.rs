//! Host adapters for canonical TinyChannels outbound messages.

use tinychannels_bus::SendMessage;

use super::Channel;

/// Build a text message addressed to one channel conversation.
pub(crate) fn text_message(
    _channel: &str,
    conversation: &str,
    text: impl Into<String>,
    thread_id: Option<String>,
) -> SendMessage {
    SendMessage::new(text, conversation)
        .in_thread(thread_id)
        .with_idempotency_key(uuid::Uuid::new_v4().to_string())
}

/// Start a draft through the only progressive draft method exposed by the
/// pinned v22 Channel trait. The contract still accepts its v21 text DTO here.
#[allow(
    deprecated,
    reason = "TinyChannels v0.1.39 exposes no typed v22 send-draft trait method"
)]
pub(crate) async fn send_draft(
    channel: &dyn Channel,
    conversation: &str,
    text: &str,
    thread_id: Option<String>,
) -> anyhow::Result<Option<String>> {
    channel
        .send_draft(&tinychannels_bus::SendMessage::new(text, conversation).in_thread(thread_id))
        .await
}
