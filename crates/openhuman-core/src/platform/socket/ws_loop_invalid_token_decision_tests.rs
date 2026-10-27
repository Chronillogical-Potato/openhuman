use super::*;

// ── decide_after_invalid_token ─────────────────────────────────────────────

/// Provider returns a genuinely fresh token → loop should retry immediately,
/// carrying the validated fresh token forward so the next attempt sends
/// exactly that value instead of re-reading the provider.
#[tokio::test]
async fn decide_after_invalid_token_fresh_token_returns_retry() {
    let provider: TokenProvider = Arc::new(|| Box::pin(async { Ok("fresh-token".to_string()) }));
    match decide_after_invalid_token("stale-token", &provider).await {
        InvalidTokenAction::RetryImmediately { token } => {
            assert_eq!(token, "fresh-token");
        }
        InvalidTokenAction::Escalate { reason } => {
            panic!("expected RetryImmediately, got Escalate({reason})");
        }
    }
}

/// Provider returns the same token → session is definitively expired; escalate.
#[tokio::test]
async fn decide_after_invalid_token_same_token_escalates() {
    let provider: TokenProvider = Arc::new(|| Box::pin(async { Ok("same-token".to_string()) }));
    match decide_after_invalid_token("same-token", &provider).await {
        InvalidTokenAction::Escalate { .. } => {}
        InvalidTokenAction::RetryImmediately { .. } => {
            panic!("expected Escalate when provider returns the same token");
        }
    }
}

/// Provider returns an error → escalate with the provider error as reason.
#[tokio::test]
async fn decide_after_invalid_token_provider_error_escalates() {
    let provider: TokenProvider = Arc::new(|| {
        Box::pin(async { Err("no session token stored — user must log in first".to_string()) })
    });
    match decide_after_invalid_token("any-token", &provider).await {
        InvalidTokenAction::Escalate { reason } => {
            assert!(
                reason.contains("provider error"),
                "expected 'provider error' in escalation reason, got: {reason}"
            );
        }
        InvalidTokenAction::RetryImmediately { .. } => {
            panic!("expected Escalate when provider errors");
        }
    }
}

/// Provider returns an empty string → treat as no session; escalate.
#[tokio::test]
async fn decide_after_invalid_token_empty_token_escalates() {
    let provider: TokenProvider = Arc::new(|| Box::pin(async { Ok(String::new()) }));
    match decide_after_invalid_token("prev-token", &provider).await {
        InvalidTokenAction::Escalate { .. } => {}
        InvalidTokenAction::RetryImmediately { .. } => {
            panic!("expected Escalate when provider returns empty token");
        }
    }
}
