use super::*;
use crate::platform::cost::tracker_test_lock;
use crate::platform::cost::types::CostRecord;
use tempfile::TempDir;

fn make_usage(input: u64, output: u64, charged: f64) -> UsageInfo {
    UsageInfo {
        input_tokens: input,
        output_tokens: output,
        context_window: 0,
        cached_input_tokens: 0,
        cache_creation_tokens: 0,
        reasoning_tokens: 0,
        charged_amount_usd: charged,
    }
}

#[test]
fn build_token_usage_skips_all_zero_payloads() {
    let usage = make_usage(0, 0, 0.0);
    assert!(build_token_usage("model-a", &usage).is_none());
}

#[test]
fn build_token_usage_populates_fields_and_total() {
    let usage = make_usage(1000, 500, 1.25);
    let translated = build_token_usage("anthropic/claude-sonnet-4", &usage).unwrap();
    assert_eq!(translated.model, "anthropic/claude-sonnet-4");
    assert_eq!(translated.input_tokens, 1000);
    assert_eq!(translated.output_tokens, 500);
    assert_eq!(translated.total_tokens, 1500);
    assert!((translated.cost_usd - 1.25).abs() < f64::EPSILON);
}

#[test]
fn build_token_usage_clamps_nan_and_negative_cost_to_zero() {
    let nan_usage = make_usage(10, 5, f64::NAN);
    let neg_usage = make_usage(10, 5, -3.0);
    let inf_usage = make_usage(10, 5, f64::INFINITY);
    assert_eq!(build_token_usage("m", &nan_usage).unwrap().cost_usd, 0.0);
    assert_eq!(build_token_usage("m", &neg_usage).unwrap().cost_usd, 0.0);
    assert_eq!(build_token_usage("m", &inf_usage).unwrap().cost_usd, 0.0);
}

#[test]
fn build_token_usage_emits_when_tokens_present_even_with_zero_cost() {
    let usage = make_usage(100, 50, 0.0);
    assert!(build_token_usage("m", &usage).is_some());
}

/// Install a global tracker if this process does not already have one, and
/// return the live instance.
///
/// The tracker is a process-wide `OnceCell` shared by every test in this
/// binary — including the runtime-bootstrap tests, which call
/// `platform::cost::init_global` through `core/jsonrpc.rs`. A test therefore
/// cannot assume it owns the global, and **cannot assume the global is
/// absent**: that is precisely why the two tests below assert on records
/// written through whichever tracker is installed, rather than on
/// `try_global()` being `None`. An assertion predicated on emptiness would be
/// order-dependent, and order-dependent is how a guard quietly becomes
/// decorative.
fn global_tracker() -> std::sync::Arc<crate::platform::cost::tracker::CostTracker> {
    let tmp = TempDir::new().unwrap();
    let mut cfg = CostConfig::default();
    cfg.enabled = true;
    init_global(cfg, tmp.path());
    // Keep the directory alive for the call above; records are re-created on
    // write (`CostStorage::add_record` re-makes the path), so a dropped temp
    // dir cannot make the read-backs below flaky.
    std::mem::forget(tmp);
    try_global().expect("init_global must leave a tracker installed")
}

/// Records written by this test only. Every test in this file shares one
/// tracker and one `costs.jsonl`, and libtest runs them in parallel, so each
/// test filters by a model name no other test uses.
fn records_for_model(
    tracker: &crate::platform::cost::tracker::CostTracker,
    model: &str,
) -> Vec<CostRecord> {
    tracker
        .get_recent_records(2, 1000)
        .expect("reading recent cost records must succeed")
        .into_iter()
        .filter(|record| record.usage.model == model)
        .collect()
}

#[test]
fn record_provider_usage_persists_through_the_global_tracker() {
    // Serialised against every other test that touches the process-global
    // tracker; see `tracker_test_lock`.
    let _lock = tracker_test_lock();
    const MODEL: &str = "test-model/record-provider-usage-persists";
    let tracker = global_tracker();

    record_provider_usage(MODEL, &make_usage(10, 5, 0.5));

    let records = records_for_model(&tracker, MODEL);
    assert_eq!(
        records.len(),
        1,
        "record_provider_usage must persist exactly one record through the global tracker"
    );
    let usage = &records[0].usage;
    assert_eq!(usage.input_tokens, 10);
    assert_eq!(usage.output_tokens, 5);
    assert_eq!(usage.total_tokens, 15);
    assert!((usage.cost_usd - 0.5).abs() < f64::EPSILON);
}

#[test]
fn record_provider_usage_skips_all_zero_payload() {
    // Serialised against every other test that touches the process-global
    // tracker; see `tracker_test_lock`.
    let _lock = tracker_test_lock();
    const MODEL: &str = "test-model/record-provider-usage-skips-zero";
    let tracker = global_tracker();

    // The all-zero skip lives in `record_provider_usage` itself (the
    // `build_token_usage` `None` arm), so it holds whether or not a tracker is
    // installed — which is what makes this the deterministic half of the old
    // `..._without_global_is_noop` test. That test asserted nothing at all, and
    // its premise ("no GLOBAL_TRACKER initialised in this test process") is not
    // something a shared-process test binary can guarantee.
    record_provider_usage(MODEL, &make_usage(0, 0, 0.0));

    assert!(
        records_for_model(&tracker, MODEL).is_empty(),
        "an all-zero usage payload must not reach the tracker"
    );
}

#[test]
fn init_global_is_idempotent() {
    // Serialised against every other test that touches the process-global
    // tracker; see `tracker_test_lock`.
    let _lock = tracker_test_lock();
    // A second `init_global` must be a no-op and must preserve the tracker the
    // first caller installed (`global.rs` early-returns when the cell is set).
    //
    // Asserting that through `try_global()` alone would be vacuous: the cell is
    // a `OnceCell`, so `set` refuses the replacement even with the early return
    // deleted, and the *pointer* stays equal either way. The observable that
    // actually distinguishes the two is one level down — `CostTracker::new`
    // eagerly creates `<workspace>/state/` (`CostStorage::new` ->
    // `fs::create_dir_all`). So if the guard is removed, the second call
    // constructs a tracker for the second workspace and that directory appears
    // on disk, even though the instance is then discarded.
    //
    // The test therefore asserts on the filesystem, which is where removing the
    // guard is visible, and on pointer identity, which catches a different
    // regression (swapping the `OnceCell` for storage that can be overwritten).
    let first_dir = TempDir::new().unwrap();
    let second_dir = TempDir::new().unwrap();
    let mut cfg = CostConfig::default();
    cfg.enabled = true;

    // After this the process has a tracker — ours, or one another test in this
    // binary installed first. Either way the *second* call below must no-op, so
    // this assertion holds regardless of test order.
    init_global(cfg.clone(), first_dir.path());
    let before = try_global().expect("init_global must leave a tracker installed");

    let second_state_dir = second_dir.path().join("state");
    assert!(
        !second_state_dir.exists(),
        "second workspace must start clean for this test to mean anything"
    );

    init_global(cfg, second_dir.path());

    assert!(
        !second_state_dir.exists(),
        "second init_global constructed a CostTracker for the second workspace: the \
         idempotence guard in init_global is gone"
    );
    let after = try_global().expect("the tracker must survive a second init_global");
    assert!(
        std::sync::Arc::ptr_eq(&before, &after),
        "second init_global replaced the installed tracker instance"
    );
}
