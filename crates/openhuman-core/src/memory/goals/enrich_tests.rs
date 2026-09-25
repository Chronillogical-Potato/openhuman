use super::*;

#[test]
fn first_run_prompt_requests_initial_population() {
    let p = build_prompt("user wants to learn rust", true);
    assert!(p.contains("EMPTY"));
    assert!(p.contains("first run"));
    assert!(p.contains("user wants to learn rust"));
}

#[test]
fn maintenance_prompt_requests_minimal_changes() {
    let p = build_prompt("user finished onboarding", false);
    assert!(p.contains("MINIMAL"));
    assert!(!p.contains("first run"));
    assert!(p.contains("user finished onboarding"));
}

// ── A7: an empty goals store is not a storage failure ───────────────────────
//
// Matrix 8.5.2 "Goals enrichment (reflect)" was 🟡. The two tests above assert
// `build_prompt` given a `first_run` bool; nothing asserted how that bool is
// DERIVED, and that derivation is where the interesting failure lives.
//
// `enrich_goals` calls `read_goals()` and takes `doc.is_empty()` as `first_run`.
// The function's comment states the invariant from the failure side:
//
//     Surface real storage failures instead of masking them as an empty
//     first-run doc. The distinction still holds through the family: a driver
//     with no goals yet answers an empty `GoalsDoc` rather than `NotFound`, so
//     an `Err` here is a real backend failure and never "the file is missing".
//
// That sentence has two halves. This test pins the half that is reachable
// without a fault-injection seam: **an empty store must not be reported as a
// load failure**. If `read_goals` ever started mapping "no goals yet" onto
// `Err`, enrichment would refuse to run for exactly the users it exists to
// help — everyone who has not set a goal yet — and the two prompt tests above
// would stay green, because they never call `read_goals` at all.
//
// The other half (a REAL backend failure must propagate rather than becoming a
// first run) is deliberately NOT asserted here. Reaching it needs a goals
// driver that errors on demand, and no such seam exists in this module today;
// an `is_err()` assertion would pass on the unrelated model-provider error this
// fixture actually produces and would prove nothing. Recorded as a gap rather
// than written vacuously.

/// An empty goals store drives the first-run path, not a load error.
#[tokio::test]
async fn an_empty_goals_store_is_not_reported_as_a_load_failure() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config = crate::config::Config {
        workspace_dir: tmp.path().to_path_buf(),
        action_dir: tmp.path().to_path_buf(),
        config_path: tmp.path().join("config.toml"),
        ..crate::config::Config::default()
    };

    // No model is configured, so the run cannot complete — that is fine and is
    // not what is under test. What matters is WHERE it stops: past `read_goals`
    // (an empty store was accepted) rather than at it.
    let error = match enrich_goals(&config, tmp.path(), "a session recap").await {
        Err(error) => error,
        // If a future fixture does let the turn complete, the invariant still
        // held — it got past the read.
        Ok(_) => return,
    };

    assert!(
        !error.starts_with("goals load failed:"),
        "an empty goals store was reported as a storage failure ({error:?}). A driver with no \
         goals yet answers an empty GoalsDoc rather than NotFound, so enrichment must treat it \
         as a first run. Mapping it to Err refuses enrichment for every user who has not set a \
         goal yet — the exact population the feature exists for."
    );
}
