use super::*;
use crate::core::socketio::GuardrailReason;

/// `is_guardrail_error_message` recognizes the `GUARDRAIL:` sentinel and
/// nothing else — mirrors `is_backend_unavailable_message`'s contract.
#[test]
fn is_guardrail_error_message_matches_only_the_sentinel() {
    assert!(is_guardrail_error_message("GUARDRAIL:{}"));
    assert!(is_guardrail_error_message(
        r#"GUARDRAIL:{"verdict":"block","score":0.9,"reasons":[]}"#
    ));
    assert!(!is_guardrail_error_message("not a guardrail error"));
    assert!(!is_guardrail_error_message(""));
    // A message that merely mentions the word must not match — only the
    // leading sentinel counts.
    assert!(!is_guardrail_error_message("this GUARDRAIL: is not at the start"));
}

/// `From<StartChatError> for String` on the `Other` variant passes the
/// message through unchanged — every pre-existing `.to_string()`/`{err}`
/// consumer of the old `Result<String, String>` `start_chat` must see
/// identical text after the `StartChatError` migration.
#[test]
fn other_variant_converts_to_string_unchanged() {
    let error = StartChatError::Other("client_id is required".to_string());
    let message: String = error.into();
    assert_eq!(message, "client_id is required");
}

/// `From<StartChatError> for String` on `Guardrail` produces the
/// `GUARDRAIL:` sentinel followed by a JSON `GuardrailPayload` that decodes
/// back to the original verdict/score/reasons — this is the RPC-surface
/// encoding `channel_web_chat`'s `?` conversion (and any other
/// string-error caller) sees.
#[test]
fn guardrail_variant_converts_to_sentinel_plus_json_payload() {
    let error = StartChatError::Guardrail {
        verdict: "block".to_string(),
        score: 0.87,
        reasons: vec![GuardrailReason {
            code: "prompt_injection".to_string(),
            message: "detected embedded instruction override".to_string(),
        }],
    };
    let message: String = error.into();
    assert!(
        message.starts_with(GUARDRAIL_ERROR_PREFIX),
        "must start with the sentinel prefix, got: {message}"
    );
    assert!(is_guardrail_error_message(&message));

    let json_part = message.strip_prefix(GUARDRAIL_ERROR_PREFIX).unwrap();
    let payload: crate::core::socketio::GuardrailPayload =
        serde_json::from_str(json_part).expect("payload must be valid JSON");
    assert_eq!(payload.verdict, "block");
    assert_eq!(payload.score, 0.87);
    assert_eq!(payload.reasons.len(), 1);
    assert_eq!(payload.reasons[0].code, "prompt_injection");
}

/// `Display` on `Guardrail` reuses the same human-readable copy a fresh
/// (non-error) rejection gets, keyed off the verdict string, so an existing
/// `.to_string()`/`{err}` consumer sees an actionable message rather than a
/// bare verdict/score dump.
#[test]
fn guardrail_display_uses_verdict_specific_user_message() {
    let blocked = StartChatError::Guardrail {
        verdict: "block".to_string(),
        score: 1.0,
        reasons: vec![],
    };
    let review_blocked = StartChatError::Guardrail {
        verdict: "review_blocked".to_string(),
        score: 0.5,
        reasons: vec![],
    };
    // Different verdicts must not collapse onto the same copy.
    assert_ne!(blocked.to_string(), review_blocked.to_string());
    assert!(!blocked.to_string().is_empty());
    assert!(!review_blocked.to_string().is_empty());
}

/// `From<&str>`/`From<String>` still build the plain `Other` variant, so
/// every internal `Err("...".to_string())` site that migrated to
/// `StartChatError` keeps compiling and behaving identically via `?`.
#[test]
fn plain_string_conversions_build_other_variant() {
    let from_owned: StartChatError = "thread_id is required".to_string().into();
    assert!(matches!(from_owned, StartChatError::Other(ref m) if m == "thread_id is required"));

    let from_borrowed: StartChatError = "message is required".into();
    assert!(matches!(from_borrowed, StartChatError::Other(ref m) if m == "message is required"));
}
