use super::*;

#[tokio::test]
async fn without_a_backend_every_flow_is_local() {
    // The lib test binary never installs a backend into the process slot.
    if crate::storage::installed().is_some() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let config = Config {
        workspace_dir: dir.path().to_path_buf(),
        ..Config::default()
    };
    assert_eq!(flow_owner(&config, "any-flow").await, None);
}

#[test]
fn a_forgotten_owner_is_resolved_again() {
    OWNERS
        .lock()
        .unwrap()
        .insert("owner-test-flow".to_string(), Some("agent-1".to_string()));
    forget("owner-test-flow");
    assert!(!OWNERS.lock().unwrap().contains_key("owner-test-flow"));
}
