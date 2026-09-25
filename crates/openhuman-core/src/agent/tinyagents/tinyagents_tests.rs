//! Native turn-model source coverage.

use std::sync::Arc;

use super::*;

#[test]
fn crate_native_turn_source_retains_only_role_and_config() {
    let source =
        TurnModelSource::new_crate_native("chat", Arc::new(crate::config::Config::default()));

    assert!(source.direct_model.is_none());
    assert!(source.crate_native.is_some());
}

#[test]
fn crate_native_text_mode_is_recorded_without_resolving_a_model() {
    let source =
        TurnModelSource::new_crate_native("chat", Arc::new(crate::config::Config::default()))
            .with_text_mode();

    assert!(source
        .crate_native
        .as_ref()
        .is_some_and(|native| native.force_text_mode));
}

#[test]
fn crate_native_text_mode_disables_native_tools_on_workload_fallbacks() {
    use crate::config::schema::cloud_providers::{AuthStyle, CloudProviderCreds};

    let _guard = crate::inference::inference_test_guard();
    let provider = "deepseek:deepseek-chat".to_string();
    let mut config = crate::config::Config::default();
    config.cloud_providers.push(CloudProviderCreds {
        id: "p_deepseek".to_string(),
        slug: "deepseek".to_string(),
        label: "DeepSeek".to_string(),
        endpoint: "https://api.deepseek.com/v1".to_string(),
        auth_style: AuthStyle::Bearer,
        default_model: Some("deepseek-chat".to_string()),
        ..Default::default()
    });
    config.chat_provider = Some(provider.clone());
    config.reasoning_provider = Some(provider.clone());
    config.agentic_provider = Some(provider.clone());
    config.coding_provider = Some(provider.clone());
    config.vision_provider = Some(provider.clone());
    config.memory_provider = Some(provider);

    let models = TurnModelSource::new_crate_native("chat", Arc::new(config))
        .with_text_mode()
        .build("chat-v1", 0.0, Some(32_000), None)
        .expect("text-mode turn models build");

    assert!(
        !models.routes.is_empty(),
        "expected workload fallback models"
    );
    assert!(
        models
            .routes
            .iter()
            .all(|(_, model)| { model.profile().is_some_and(|profile| !profile.tool_calling) }),
        "every workload fallback must preserve prompt-guided text mode"
    );
}

#[test]
fn direct_model_turn_source_builds_without_provider_adapter() {
    let model: Arc<dyn tinyinference_llm::model::ChatModel<()>> =
        Arc::new(tinyagents_harness::testkit::ScriptedModel::replies(vec![
            "done",
        ]));
    let source = TurnModelSource::from_model(model);

    assert!(source.crate_native.is_none());
    assert!(source.direct_model.is_some());

    let models = source
        .build("mock-model", 0.0, Some(32_000), None)
        .expect("direct model source builds");
    assert_eq!(models.provider_id(), "injected");
    assert_eq!(models.context_window(), Some(32_000));
    assert!(!models.native_tools());
}

#[test]
fn run_policy_for_makes_invalid_tool_arguments_recoverable() {
    let policy = run_policy_for(10, false);
    assert_eq!(
        policy.invalid_args,
        InvalidArgsPolicy::ReturnToolError,
        "schema-invalid calls must return a corrective tool result instead of aborting the turn"
    );
}

#[test]
fn run_policy_retries_one_nontruncated_empty_completion() {
    let policy = run_policy_for(10, false);
    assert_eq!(policy.empty_response_retries, 1);
}

#[test]
fn parse_model_call_wall_clock_defaults_to_fifteen_minutes() {
    // Absent and unparseable values both fall back to the 900s default.
    assert_eq!(parse_model_call_wall_clock_ms(None), Some(900_000));
    assert_eq!(
        parse_model_call_wall_clock_ms(Some("garbage")),
        Some(900_000)
    );
    assert_eq!(parse_model_call_wall_clock_ms(Some("")), Some(900_000));
}

#[test]
fn parse_model_call_wall_clock_honors_explicit_value_and_zero_opt_out() {
    assert_eq!(parse_model_call_wall_clock_ms(Some("300")), Some(300_000));
    assert_eq!(parse_model_call_wall_clock_ms(Some(" 300 ")), Some(300_000));
    // `0` disables the per-call ceiling entirely (remainder-only, pre-#5766).
    assert_eq!(parse_model_call_wall_clock_ms(Some("0")), None);
}

#[test]
fn run_policy_wires_both_wall_clock_ceilings_from_their_resolvers() {
    // Compare the policy against the same-process resolvers rather than
    // hard-coded defaults, so a dev/CI environment exporting either
    // `OPENHUMAN_MODEL_CALL_TIMEOUT_SECS` or `OPENHUMAN_AGENT_TURN_TIMEOUT_SECS`
    // (including `0` = disabled, or a per-call value above the turn value)
    // cannot fail the test while the wiring is correct. No env mutation —
    // whatever the environment says, the policy must carry exactly what the
    // resolvers produce.
    let policy = run_policy_for(10, false);
    assert_eq!(policy.limits.max_model_call_ms, model_call_wall_clock_ms());
    assert_eq!(policy.limits.max_wall_clock_ms, agent_turn_wall_clock_ms());
}

#[test]
fn default_per_call_ceiling_sits_under_the_default_turn_ceiling() {
    // Env-free: assert the *defaults* through the pure parsers, not through
    // the env-reading policy path. A per-call ceiling at or above the turn
    // deadline is dead code, because `min(ceiling, remainder)` would always
    // pick the remainder.
    let per_call =
        parse_model_call_wall_clock_ms(None).expect("the per-call ceiling is armed by default");
    let turn = parse_agent_turn_wall_clock_ms(None).expect("the turn ceiling is armed by default");
    assert_eq!(per_call, DEFAULT_MODEL_CALL_TIMEOUT_SECS * 1_000);
    assert_eq!(turn, DEFAULT_AGENT_TURN_TIMEOUT_SECS * 1_000);
    assert!(
        per_call < turn,
        "per-call ceiling ({per_call} ms) must be under the turn ceiling ({turn} ms)"
    );
}

/// #6413: the retry schedule must outlast a throttled upstream.
///
/// These pin the two numbers that were wrong — the attempt count and the total
/// time spent waiting — rather than "it retried", which passed on the old
/// 2-retry/1.5 s schedule and would pass again if someone shortened it.
#[test]
fn run_policy_retry_schedule_outlasts_a_throttled_upstream() {
    use tinyagents_harness::retry::JITTER_FRACTION;

    let policy = run_policy_for(10, false);
    let retry = &policy.retry;

    // 4 retries (5 attempts). The issue asks for "at least 3".
    assert_eq!(retry.max_attempts, 5, "attempts (first try + retries)");
    assert!(
        retry.backoff_sleep,
        "a retry that does not wait is not a backoff"
    );

    // The harness applies `max_attempts_capped_at(limits.max_retries_per_call)`
    // at the model call, and `RunLimits` defaults that to 3 — so without this the
    // policy above is silently clamped to 4 attempts and the change is a no-op.
    // This is the assertion that catches a "fix" that raised only `max_attempts`.
    assert!(
        policy.limits.max_retries_per_call + 1 >= retry.max_attempts,
        "max_attempts {} is clamped by max_retries_per_call {} — the schedule would \
         silently lose an attempt",
        retry.max_attempts,
        policy.limits.max_retries_per_call,
    );

    // Total time actually spent waiting, on the WORST jitter draw. `rand01 = 0.0`
    // is the bottom of the band (`base * (1 - JITTER_FRACTION)`), which is the
    // figure that has to clear the issue's floor — the nominal schedule clearing
    // it while the floor does not would be an intermittent failure to retry long
    // enough, and those are the ones nobody reproduces.
    let retries = retry.max_attempts - 1;
    let worst: std::time::Duration = (0..retries)
        .map(|attempt| retry.backoff_for_attempt_with(attempt, 0.0))
        .sum();
    assert!(
        worst >= std::time::Duration::from_secs(30),
        "worst-case backoff across {retries} retries is {:?}; #6413 requires >= 30s",
        worst,
    );

    // And the nominal (mid-band) schedule, so a future edit that satisfies the
    // floor by widening jitter rather than lengthening the curve still fails.
    let nominal: std::time::Duration = (0..retries)
        .map(|attempt| retry.backoff_for_attempt_with(attempt, 0.5))
        .sum();
    assert!(
        nominal >= std::time::Duration::from_secs(40),
        "nominal backoff is {nominal:?}; expected >= 40s so the jittered floor has margin",
    );

    // Jitter on: a throttled fleet must not retry in lockstep on one curve.
    assert!(
        retry.jitter,
        "identical curves keep a saturated upstream saturated"
    );
    assert!(JITTER_FRACTION > 0.0, "jitter must actually widen the band");

    // Both ceilings stay bounded — this must not become an unbounded retry
    // (cf. #6412, the opposite failure in another crate).
    assert!(retry.max_attempts < 10, "attempts must stay bounded");
    assert!(
        retry.max_retry_after_ms > 0 && retry.max_retry_after_ms <= 300_000,
        "a server Retry-After must be honoured but capped, not obeyed without limit"
    );
}
