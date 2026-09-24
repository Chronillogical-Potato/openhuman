use super::*;

// ── TurnCostThrottle ───────────────────────────────────────────────────────

/// The first `should_emit()` call for a fresh turn always returns `true` so
/// the initial cost readout reaches the client immediately, without waiting
/// out `TURN_COST_EMIT_MIN_INTERVAL`.
#[test]
fn turn_cost_throttle_always_emits_first_time() {
    let mut throttle = TurnCostThrottle::new();
    assert!(throttle.should_emit(), "first call must always emit");
}

/// A second call immediately after the first is suppressed — the throttle
/// enforces `TURN_COST_EMIT_MIN_INTERVAL` (750ms) between emissions so a
/// fast-tool-calling round doesn't repaint the cost readout several times a
/// second.
#[test]
fn turn_cost_throttle_suppresses_rapid_followup() {
    let mut throttle = TurnCostThrottle::new();
    assert!(throttle.should_emit(), "first call must emit");
    assert!(
        !throttle.should_emit(),
        "an immediate second call must be suppressed"
    );
    assert!(
        !throttle.should_emit(),
        "a third rapid call must still be suppressed"
    );
}

/// Once the minimum interval has elapsed, the throttle allows another
/// emission and resets its clock.
#[test]
fn turn_cost_throttle_emits_again_after_interval_elapses() {
    let mut throttle = TurnCostThrottle::new();
    assert!(throttle.should_emit(), "first call must emit");
    assert!(!throttle.should_emit(), "immediate followup suppressed");

    // Fast-forward past the throttle window by backdating `last_emit`
    // directly rather than sleeping the test for 750ms.
    throttle.last_emit = Some(
        std::time::Instant::now()
            - TURN_COST_EMIT_MIN_INTERVAL
            - std::time::Duration::from_millis(1),
    );
    assert!(
        throttle.should_emit(),
        "must emit again once the interval has elapsed"
    );
}

// ── TurnTimingSnapshot::into_payload ───────────────────────────────────────

/// `tokens_per_second` is computed from `output_tokens / (total_ms / 1000)`
/// when both are available and `total_ms > 0`.
#[test]
fn into_payload_computes_tokens_per_second_when_available() {
    let snapshot = TurnTimingSnapshot {
        first_token_ms: Some(100),
        first_tool_ms: None,
        total_ms: Some(2000),
    };
    let payload = snapshot.into_payload(Some(40));
    assert_eq!(payload.first_token_ms, Some(100));
    assert_eq!(payload.first_tool_ms, None);
    assert_eq!(payload.total_ms, Some(2000));
    // 40 tokens / (2000ms / 1000) = 20 tokens/sec.
    assert_eq!(payload.tokens_per_second, Some(20.0));
}

/// No `output_tokens` → no `tokens_per_second`, even with a valid `total_ms`.
#[test]
fn into_payload_omits_tokens_per_second_without_output_tokens() {
    let snapshot = TurnTimingSnapshot {
        first_token_ms: Some(50),
        first_tool_ms: Some(75),
        total_ms: Some(1000),
    };
    let payload = snapshot.into_payload(None);
    assert_eq!(payload.tokens_per_second, None);
}

/// `total_ms == Some(0)` must not divide by zero — `tokens_per_second` stays
/// `None` rather than producing infinity on an instantaneous synthetic
/// result.
#[test]
fn into_payload_guards_against_division_by_zero_total_ms() {
    let snapshot = TurnTimingSnapshot {
        first_token_ms: None,
        first_tool_ms: None,
        total_ms: Some(0),
    };
    let payload = snapshot.into_payload(Some(10));
    assert_eq!(payload.tokens_per_second, None);
}

/// No `total_ms` at all (turn never reached `TurnCompleted`) → no
/// `tokens_per_second`.
#[test]
fn into_payload_omits_tokens_per_second_without_total_ms() {
    let snapshot = TurnTimingSnapshot {
        first_token_ms: Some(10),
        first_tool_ms: None,
        total_ms: None,
    };
    let payload = snapshot.into_payload(Some(10));
    assert_eq!(payload.tokens_per_second, None);
}
