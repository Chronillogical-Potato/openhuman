use super::outbound_intent;
use tinychannels_bus::{DeliveryDurability, OutboundPayload, SendMessage};

#[test]
fn send_message_becomes_an_equivalent_intent() {
    let message = SendMessage::new("hello", "chat-1")
        .in_thread(Some("thread-1".into()))
        .with_idempotency_key("turn-1:segment-0");

    let intent = outbound_intent("telegram", &message, DeliveryDurability::Required);
    assert_eq!(intent.channel_id, "telegram");
    assert_eq!(intent.conversation_id, message.recipient);
    assert_eq!(intent.reply_to_id, None);
    assert_eq!(intent.thread_id, message.thread_ts);
    assert_eq!(intent.idempotency_key, message.idempotency_key.unwrap());
    assert_eq!(intent.durability, DeliveryDurability::Required);
    assert!(matches!(
        intent.payload,
        OutboundPayload::Text { text } if text == message.content
    ));
}
