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
fn only_mongodb_is_shared_between_processes() {
    assert!(driver_is_shared("mongodb"));
    for driver in ["sqlite", "memory", "file"] {
        assert!(!driver_is_shared(driver), "{driver}");
    }
}
