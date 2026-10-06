use super::delivery::channel_message_body_with_idempotency;
use serde_json::json;

#[test]
fn channel_message_body_adds_deterministic_idempotency_key() {
    let left = channel_message_body_with_idempotency(
        "telegram",
        json!({ "text": "hello", "threadId": "topic-1" }),
    );
    let right = channel_message_body_with_idempotency(
        "telegram",
        json!({ "threadId": "topic-1", "text": "hello" }),
    );

    assert_eq!(left["text"], "hello");
    assert_eq!(left["threadId"], "topic-1");
    assert_eq!(left["idempotencyKey"], right["idempotencyKey"]);
    assert!(left["idempotencyKey"]
        .as_str()
        .expect("idempotency key")
        .starts_with("legacy-send:telegram:"));
}

#[test]
fn channel_message_body_preserves_caller_idempotency_key() {
    let body = channel_message_body_with_idempotency(
        "discord",
        json!({ "text": "hello", "idempotencyKey": "caller-key" }),
    );

    assert_eq!(body["idempotencyKey"], "caller-key");
}
