// --- harness fan-out concurrency ceiling ---

#[test]
fn harness_ceiling_defaults_when_unset_or_nonsense() {
    // A malformed override must never produce a zero-permit semaphore —
    // that would deadlock every flow agent node in the process.
    for raw in [None, Some(""), Some("0"), Some("-4"), Some("lots")] {
        assert_eq!(
            super::max_parallel_harness_agents(raw),
            super::DEFAULT_MAX_PARALLEL_HARNESS_AGENTS,
            "{raw:?} should fall back to the default"
        );
    }
}

#[test]
fn harness_ceiling_honours_a_valid_override() {
    assert_eq!(super::max_parallel_harness_agents(Some("3")), 3);
    assert_eq!(super::max_parallel_harness_agents(Some(" 16 ")), 16);
}

#[tokio::test]
async fn production_harness_ceiling_is_open_and_reusable() {
    let held = super::HARNESS_AGENT_SLOTS
        .acquire()
        .await
        .expect("the production limiter must remain open");
    drop(held);
    assert!(!super::HARNESS_AGENT_SLOTS.is_closed());
}
