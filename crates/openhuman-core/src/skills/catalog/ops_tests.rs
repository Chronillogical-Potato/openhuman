use super::*;
use serde_json::json;

fn same_named_catalog() -> Vec<CatalogEntry> {
    [
        json!({ "name": "AI Code Review", "source": "ClawHub", "identifier": "qf-code-review" }),
        json!({ "name": "AI Code Review", "source": "ClawHub", "identifier": "ai-code-review-ops" }),
        json!({ "name": "apple-notes", "source": "built-in", "docsPath": "bundled/apple/apple-apple-notes" }),
        json!({ "name": "apple-notes", "source": "ClawHub", "identifier": "apple-notes" }),
        json!({ "name": "Apple Design", "source": "ClawHub", "identifier": "apple-design" }),
    ]
    .iter()
    .map(|item| parse_hermes_entry(item).expect("entry"))
    .collect()
}

#[test]
fn find_catalog_entry_not_found_suggests_real_ids_instead_of_a_refresh() {
    let err = find_catalog_entry(&same_named_catalog(), "ai-code-review").unwrap_err();
    assert!(
        err.starts_with("no catalog entry has id 'ai-code-review'"),
        "{err}"
    );
    assert!(err.contains("clawhub/ai-code-review-ops"), "{err}");
    assert!(!err.contains("refresh"), "{err}");
}

#[tokio::test]
async fn install_from_catalog_errors_for_portal_skill_without_download() {
    // A portal-only entry (empty download_url) must fail fast with an
    // actionable message naming the source + page — never fetch a 404. (#3741)
    let tmp = tempfile::tempdir().unwrap();
    let entry = parse_hermes_entry(&json!({
        "name": "code-audit",
        "description": "x",
        "category": "other",
        "source": "ClawHub",
        "sourceUrl": "https://clawhub.ai/skills/agentkilox-code-audit"
    }))
    .expect("entry");
    assert_eq!(entry.download_url, "");

    let err = install_from_catalog(tmp.path(), &entry)
        .await
        .expect_err("portal skill cannot install");
    assert!(err.contains("ClawHub"), "names the source: {err}");
    assert!(
        err.contains("https://clawhub.ai/skills/agentkilox-code-audit"),
        "links the source page: {err}"
    );
}

#[test]
fn refresh_on_boot_enabled_defaults_on_and_accepts_common_false_values() {
    assert!(refresh_on_boot_enabled(None));
    assert!(refresh_on_boot_enabled(Some("1")));
    assert!(refresh_on_boot_enabled(Some("true")));

    assert!(!refresh_on_boot_enabled(Some("0")));
    assert!(!refresh_on_boot_enabled(Some("false")));
    assert!(!refresh_on_boot_enabled(Some(" no ")));
    assert!(!refresh_on_boot_enabled(Some("OFF")));
}

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering as AtomicOrdering};
use std::sync::Arc;

const CACHE_DIR_ENV: &str = "OPENHUMAN_SKILL_REGISTRY_CACHE_DIR";

fn env_lock() -> tokio::sync::MutexGuard<'static, ()> {
    crate::skills::catalog::TEST_ENV_LOCK.blocking_lock()
}

async fn env_lock_async() -> tokio::sync::MutexGuard<'static, ()> {
    crate::skills::catalog::TEST_ENV_LOCK.lock().await
}

fn sample_entry() -> CatalogEntry {
    parse_hermes_entry(&json!({
        "name": "apple-notes",
        "description": "Manage Apple Notes",
        "category": "apple",
        "source": "built-in",
        "docsPath": "bundled/apple/apple-apple-notes"
    }))
    .expect("entry")
}

#[tokio::test]
async fn fresh_cache_skips_fetch() {
    let _env = env_lock_async().await;
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var(CACHE_DIR_ENV, tmp.path());
    store::save_catalog_cache(&[sample_entry()]);

    let called = Arc::new(AtomicBool::new(false));
    let called_in = called.clone();
    let entries = browse_catalog_with(false, StaleMode::Allow, move || async move {
        called_in.store(true, AtomicOrdering::SeqCst);
        Ok(Vec::new())
    })
    .await
    .unwrap();

    assert_eq!(entries.len(), 1);
    assert!(
        !called.load(AtomicOrdering::SeqCst),
        "fetcher must not run when the cache is fresh"
    );

    store::clear_cache();
    std::env::remove_var(CACHE_DIR_ENV);
}

#[tokio::test]
async fn concurrent_cache_miss_coalesces_to_single_fetch() {
    let _env = env_lock_async().await;
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var(CACHE_DIR_ENV, tmp.path());
    store::clear_cache();

    let calls = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();
    for _ in 0..4 {
        let calls = calls.clone();
        handles.push(tokio::spawn(async move {
            browse_catalog_with(false, StaleMode::Allow, move || async move {
                calls.fetch_add(1, AtomicOrdering::SeqCst);
                // Mimic the slow upstream so the other callers queue on the
                // single-flight lock instead of each starting a fetch.
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                let entries = vec![sample_entry()];
                store::save_catalog_cache(&entries);
                Ok(entries)
            })
            .await
        }));
    }

    for handle in handles {
        let entries = handle.await.unwrap().unwrap();
        assert_eq!(entries.len(), 1, "every caller receives the catalog");
    }
    assert_eq!(
        calls.load(AtomicOrdering::SeqCst),
        1,
        "four concurrent cache-miss callers must trigger exactly one fetch"
    );

    store::clear_cache();
    std::env::remove_var(CACHE_DIR_ENV);
}

/// Write a cache file with an explicit `fetched_at_epoch` (epoch 1 => stale).
fn write_cache_at(dir: &std::path::Path, entries: Vec<CatalogEntry>, epoch: u64) {
    let cache = store::CatalogCache {
        entries,
        fetched_at_epoch: epoch,
    };
    std::fs::write(
        dir.join("cache.json"),
        serde_json::to_string(&cache).unwrap(),
    )
    .unwrap();
}

#[tokio::test]
async fn browse_serves_stale_without_a_foreground_fetch() {
    let _env = env_lock_async().await;
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var(CACHE_DIR_ENV, tmp.path());
    write_cache_at(tmp.path(), vec![sample_entry()], 1); // epoch 1 => stale

    // Pin REFRESHING so the background revalidation no-ops (no real network).
    REFRESHING.store(true, AtomicOrdering::SeqCst);
    let called = Arc::new(AtomicBool::new(false));
    let called_in = called.clone();
    let entries = browse_catalog_with(false, StaleMode::Allow, move || async move {
        called_in.store(true, AtomicOrdering::SeqCst);
        Ok(Vec::new())
    })
    .await
    .unwrap();
    REFRESHING.store(false, AtomicOrdering::SeqCst);

    assert_eq!(entries.len(), 1, "browse returns the stale entry");
    assert!(
        !called.load(AtomicOrdering::SeqCst),
        "browse must serve stale without a foreground fetch"
    );

    store::clear_cache();
    std::env::remove_var(CACHE_DIR_ENV);
}

#[tokio::test]
async fn search_rejects_stale_and_fetches_fresh() {
    let _env = env_lock_async().await;
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var(CACHE_DIR_ENV, tmp.path());
    write_cache_at(tmp.path(), vec![sample_entry()], 1); // stale: 1 entry

    let called = Arc::new(AtomicBool::new(false));
    let called_in = called.clone();
    let entries = browse_catalog_with(false, StaleMode::Reject, move || async move {
        called_in.store(true, AtomicOrdering::SeqCst);
        let fresh = vec![sample_entry(), sample_entry()];
        store::save_catalog_cache(&fresh);
        Ok(fresh)
    })
    .await
    .unwrap();

    assert!(
        called.load(AtomicOrdering::SeqCst),
        "a fresh (search) read must not be satisfied by a stale cache"
    );
    assert_eq!(
        entries.len(),
        2,
        "returns the freshly fetched catalog, not the stale one"
    );

    store::clear_cache();
    std::env::remove_var(CACHE_DIR_ENV);
}

#[tokio::test]
async fn search_ranks_installable_entries_before_uninstallable_ones() {
    let _env = env_lock_async().await;
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var(CACHE_DIR_ENV, tmp.path());
    let agent = parse_hermes_entry(&json!({
        "name": "review-agent",
        "description": "x",
        "source": "LobeHub",
        "identifier": "lobehub/review-agent",
        "sourceUrl": "https://lobehub.com/agent/review-agent"
    }))
    .unwrap();
    let skill = parse_hermes_entry(&json!({
        "name": "review-skill",
        "description": "x",
        "source": "ClawHub",
        "identifier": "review-skill"
    }))
    .unwrap();
    store::save_catalog_cache(&[agent, skill]);

    let hits = search_catalog("review", None, None).await.unwrap();

    let ids: Vec<&str> = hits.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(
        ids,
        ["clawhub/review-skill", "lobehub/review-agent"],
        "an installable hit must come before one install will reject"
    );

    store::clear_cache();
    std::env::remove_var(CACHE_DIR_ENV);
}
