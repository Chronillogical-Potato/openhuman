use super::*;

// ── fresh-install provider seeding (#6205) ──────────────────────────────
//
// The case neither the original fix nor its tests covered, and the one that
// regressed: a workspace created *today*, not one migrated from an older
// schema.

/// A brand-new workspace must be born with the managed `openhuman` cloud
/// provider.
///
/// `load_or_init` stamps a fresh config with `CURRENT_SCHEMA_VERSION`, so the
/// `run_pending` call that follows crosses no gate at all — including the
/// `== 1` step that is the only place this entry has ever been seeded. Without
/// creation-site seeding the list is empty, and no later migration can repair
/// it, because the workspace is already past every gate that would have.
///
/// This is what made `inference_list_models("openhuman")` fail its lookup in
/// `inference::provider::ops::models` before any HTTP request was made, which
/// is what #6201's model picker surfaced as "Could not load models from this
/// provider."
#[tokio::test]
async fn a_brand_new_workspace_is_born_with_the_managed_provider() {
    use crate::config::schema::cloud_providers::AuthStyle;

    let tmp = tempfile::tempdir().unwrap();
    let config = load_or_init_for_workspace(tmp.path()).await;

    let managed = config
        .cloud_providers
        .iter()
        .find(|p| p.slug == "openhuman")
        .unwrap_or_else(|| {
            panic!(
                "a fresh workspace must carry the managed provider, got {:?}",
                config
                    .cloud_providers
                    .iter()
                    .map(|p| p.slug.as_str())
                    .collect::<Vec<_>>()
            )
        });
    assert_eq!(managed.auth_style, AuthStyle::OpenhumanJwt);
    assert!(
        !managed.endpoint.trim().is_empty(),
        "the seeded entry needs a resolvable endpoint"
    );

    // Seeded before the first `save`, so it is on disk from the very first
    // write — not merely in memory awaiting some later persist.
    let on_disk = tokio::fs::read_to_string(&config.config_path)
        .await
        .unwrap();
    assert!(
        on_disk.contains("slug = \"openhuman\""),
        "the seeded entry must be persisted by the initial save, got:\n{on_disk}"
    );
}

/// Re-opening that workspace must not seed a second copy.
///
/// `seed_cloud_providers` early-returns on a non-empty list, and the version
/// gate is already past, so the second load has to be a no-op. A duplicate here
/// would mean the entry is being created on a path that does not check first.
#[tokio::test]
async fn reopening_a_seeded_workspace_does_not_duplicate_the_provider() {
    let tmp = tempfile::tempdir().unwrap();
    let first = load_or_init_for_workspace(tmp.path()).await;
    let seeded = first
        .cloud_providers
        .iter()
        .filter(|p| p.slug == "openhuman")
        .count();
    assert_eq!(seeded, 1, "first load seeds exactly one managed entry");

    let second = load_or_init_for_workspace(tmp.path()).await;
    assert_eq!(
        second
            .cloud_providers
            .iter()
            .filter(|p| p.slug == "openhuman")
            .count(),
        1,
        "re-opening the workspace must not seed a second managed entry"
    );
}
