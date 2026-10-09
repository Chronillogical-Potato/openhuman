//! Tests of the hand-off from the import to organizing memory: an import
//! counts as unfinished from its reservation, and a finished import starts
//! the move into the per-user tree.

use std::sync::Arc;

use tinymemory_api::MetaFilter;

use super::tests::{legacy_workspace, wait_until_settled};
use super::*;
use crate::memory::test_fixtures::{bind_reference, config_in, stored};

#[test]
fn an_import_counts_as_in_progress_from_its_reservation() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    assert!(!in_progress(&config));
    // Reserved, still scanning the old store: no state written yet.
    RUNNING.lock().unwrap().insert(config.workspace_dir.clone());
    assert!(in_progress(&config));
    RUNNING.lock().unwrap().remove(&config.workspace_dir);
    assert!(!in_progress(&config));
}

#[tokio::test]
async fn a_finished_import_starts_organizing_into_the_per_user_tree() {
    let tmp = tempfile::tempdir().unwrap();
    let mut config = config_in(&tmp);
    let account = "0123456789abcdef01234567";
    // Signed in: the move needs the account's own root, and the switch
    // writes the account's config file.
    config.config_path = tmp.path().join("users").join(account).join("config.toml");
    std::fs::create_dir_all(config.config_path.parent().unwrap()).unwrap();
    config.save().await.unwrap();
    legacy_workspace(&config.workspace_dir);
    let legacy = bind_reference(&config);
    let tree = Arc::new(tinymemory_api::conformance::ReferenceEngine::new());
    crate::memory::engine::install_test_engine_for_root(
        &config.workspace_dir,
        Some(&format!("org:{account}")),
        tree.clone(),
    );

    start(&config, true).await.unwrap();
    assert_eq!(wait_until_settled(&config).await.phase, ImportPhase::Done);

    // The import's own task starts the move once it is done.
    let mut moved = None;
    for _ in 0..400 {
        let migration = crate::memory::layout_migration::status(&config).unwrap();
        if !migration.running
            && migration.state.phase == crate::memory::layout_migration::Phase::Cleaned
        {
            moved = Some(migration.state);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let moved = moved.expect("the move never finished after the import");
    assert_eq!(moved.copied, 5);
    assert_eq!(stored(&tree, MetaFilter::default()).await.len(), 5);
    assert!(stored(&legacy, MetaFilter::default()).await.is_empty());
}
