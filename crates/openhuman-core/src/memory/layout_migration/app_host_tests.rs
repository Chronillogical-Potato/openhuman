use std::sync::Arc;

use tinymemory_api::conformance::ReferenceEngine;
use tinymemory_api::MemoryEngine;

use super::*;
use crate::memory::engine::install_test_engine_for_root;
use crate::memory::layout_migration::service::scan;
use crate::memory::layout_migration::test_host::{count, fact};

const ACCOUNT: &str = "0123456789abcdef01234567";

/// A config that lives where a signed-in account's does.
fn signed_in(tmp: &tempfile::TempDir) -> Config {
    let mut config = crate::memory::test_fixtures::config_in(tmp);
    config.config_path = tmp.path().join("users").join(ACCOUNT).join("config.toml");
    config
}

#[tokio::test]
async fn signed_out_there_is_nothing_to_move() {
    let tmp = tempfile::tempdir().unwrap();
    let config = crate::memory::test_fixtures::config_in(&tmp);
    assert!(matches!(AppHost.engines(&config), Err(MemoryError::Off(_))));
    assert!(!scan(&config, &AppHost).await.unwrap().needed);
}

#[tokio::test]
async fn binds_the_legacy_tree_and_the_users_own() {
    let tmp = tempfile::tempdir().unwrap();
    let config = signed_in(&tmp);
    let root = format!("user:{ACCOUNT}");
    let legacy = Arc::new(ReferenceEngine::new());
    let tree = Arc::new(ReferenceEngine::new());
    install_test_engine_for_root(&config.workspace_dir, None, legacy.clone());
    install_test_engine_for_root(&config.workspace_dir, Some(&root), tree.clone());
    legacy.store(fact("legacy fact")).await.unwrap();

    let engines = AppHost.engines(&config).unwrap();
    assert_eq!(count(&*engines.legacy).await, 1);
    assert_eq!(count(&*engines.tree).await, 0);
    assert!(scan(&config, &AppHost).await.unwrap().needed);

    let placement = AppHost.placement(&config).unwrap();
    assert!(placement.chat_node.to_string().ends_with("ws:main"));
    assert!(matches!(placement.flows, FlowPlacement::WithRoot));
}
