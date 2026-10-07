use super::*;

#[tokio::test]
async fn per_turn_tool_limit_reaches_the_execution_policy() {
    crate::agent::stop_hooks::with_tool_call_limit(Some(2), async {
        let policy = run_policy_for(10, false);
        assert_eq!(policy.limits.max_tool_calls, 2);
        assert_eq!(policy.limits.max_model_calls, 10);
    })
    .await;
    assert_eq!(run_policy_for(10, false).limits.max_tool_calls, 80);
}

/// #6953: the model reads how long each tool call took, so it can budget the
/// rest of the turn against it.
#[test]
fn tool_results_carry_their_duration() {
    assert!(run_policy_for(10, false).tool_result_durations);
}

#[test]
fn stream_timeouts_are_set_explicitly_with_first_event_off() {
    let mut limits = RunPolicy::default().limits;
    apply_stream_limits(&mut limits, None, None, None);
    assert_eq!(limits.stream_idle_timeout_ms, Some(120_000));
    assert_eq!(limits.stream_first_event_timeout_ms, None);
    assert_eq!(limits.max_consecutive_stream_idle_timeouts, Some(5));
}

#[test]
fn stream_timeout_env_overrides_parse() {
    assert_eq!(parse_stream_idle_timeout_ms(Some("30")), Some(30_000));
    assert_eq!(parse_stream_idle_timeout_ms(Some("0")), None);
    assert_eq!(parse_stream_idle_timeout_ms(Some("junk")), Some(120_000));
    assert_eq!(
        parse_stream_first_event_timeout_ms(Some("45")),
        Some(45_000)
    );
    assert_eq!(parse_stream_first_event_timeout_ms(Some("0")), None);
    assert_eq!(parse_stream_first_event_timeout_ms(None), None);
    assert_eq!(parse_max_consecutive_stream_idle_timeouts(Some("0")), None);
    assert_eq!(
        parse_max_consecutive_stream_idle_timeouts(Some("2")),
        Some(2)
    );
}
