use super::*;

fn config_with(url: Option<&str>) -> Config {
    let mut config = Config::default();
    config.storage.url = url.map(str::to_string);
    config
}

#[test]
fn the_environment_wins_over_the_config_file() {
    let config = config_with(Some("sqlite:/from/config"));
    assert_eq!(
        url_from(Some("memory".into()), &config).as_deref(),
        Some("memory")
    );
    assert_eq!(
        url_from(None, &config).as_deref(),
        Some("sqlite:/from/config")
    );
}

#[test]
fn blank_values_keep_the_classic_layout() {
    assert_eq!(url_from(None, &config_with(None)), None);
    assert_eq!(url_from(Some("  ".into()), &config_with(Some(""))), None);
    assert_eq!(
        url_from(Some(" ".into()), &config_with(Some(" memory "))).as_deref(),
        Some("memory"),
        "a blank override falls through to the file"
    );
}

#[tokio::test]
async fn opens_memory_and_refuses_garbage() {
    let backend = open("memory").await.unwrap();
    assert_eq!(backend.driver(), "memory");
    assert!(open("ftp://nope").await.is_err());
}

#[test]
fn install_installed_and_clear() {
    // One test owns the process slot, so it cannot race another.
    clear();
    assert!(installed().is_none());
    let backend: Arc<dyn StorageBackend> = Arc::new(tinystoragedrivers::MemoryStorage::new());
    assert!(install(Arc::clone(&backend)).is_none());
    assert!(installed().is_some_and(|got| Arc::ptr_eq(&got, &backend)));
    assert!(install(Arc::new(tinystoragedrivers::MemoryStorage::new())).is_some());
    assert!(clear());
    assert!(!clear());
}

#[test]
fn agent_scopes_match_the_session_store() {
    assert_eq!(scope_for_agent("agent-7").as_str(), "agent-7");
    assert!(scope_for_agent("has a space")
        .as_str()
        .starts_with("sha256:"));
}

#[test]
fn the_scope_follows_the_acting_agent_and_fails_closed_in_saas() {
    assert_eq!(scope_from(Some("agent-7"), false).unwrap().as_str(), "agent-7");
    assert_eq!(scope_from(Some("agent-7"), true).unwrap().as_str(), "agent-7");
    assert!(scope_from(None, false).unwrap().is_local());
    assert!(scope_from(None, true).is_err(), "SaaS never falls back to a shared scope");
}

#[test]
fn block_on_runs_a_future_from_sync_code() {
    let answer = block_on(async { Ok::<_, StorageError>(42) }).unwrap();
    assert_eq!(answer, 42);
    let error = block_on(async { Err::<(), _>(StorageError::conflict("raced")) }).unwrap_err();
    assert_eq!(error.message(), "raced");
}

#[tokio::test(flavor = "multi_thread")]
async fn block_on_works_inside_a_runtime_too() {
    let answer = tokio::task::spawn_blocking(|| block_on(async { Ok::<_, StorageError>(7) }))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(answer, 7);
}
