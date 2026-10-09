use super::*;
use std::sync::Mutex;
use tinyagents_session::testkit::conformance::session_store_conformance;
use tinyagents_session::turn_state::{TurnLifecycle, TurnState};

#[tokio::test]
async fn the_desktop_layout_meets_the_session_store_contract() {
    let dir = tempfile::tempdir().unwrap();
    session_store_conformance(&SqliteSessionStores::at(dir.path())).await;
    // Everything landed in the classic layout.
    assert!(dir.path().join("session_raw").is_dir());
    assert!(dir.path().join("tinyagents_store").is_dir());
}

#[test]
fn the_workspace_follows_the_resolver() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let current = Arc::new(Mutex::new(first.path().to_path_buf()));
    let resolver = current.clone();
    let stores = SqliteSessionStores::resolving(move || resolver.lock().unwrap().clone());
    assert_eq!(stores.workspace_dir().as_deref(), Some(first.path()));
    *current.lock().unwrap() = second.path().to_path_buf();
    assert_eq!(stores.workspace_dir().as_deref(), Some(second.path()));
    assert_eq!(
        stores.destination_key(),
        Some(second.path().to_string_lossy().into_owned())
    );
    assert!(format!("{stores:?}").contains("SqliteSessionStores"));
}

#[test]
fn recovery_interrupts_turns_left_in_flight() {
    let dir = tempfile::tempdir().unwrap();
    let stores = SqliteSessionStores::at(dir.path());
    let agent = stores.for_agent("orchestrator");
    agent
        .turn_states
        .put(&TurnState::started("t", "r", 4, "2026-01-01T00:00:00Z"))
        .unwrap();
    stores.recover().unwrap();
    assert_eq!(
        agent
            .turn_states
            .get("t")
            .unwrap()
            .map(|turn| turn.lifecycle),
        Some(TurnLifecycle::Interrupted)
    );
}

/// The provider and storage slots are process-wide; these tests take turns.
static SLOTS: Mutex<()> = Mutex::new(());

/// Puts the process-global storage backend back as a test found it.
fn restore_backend(previous: Option<Arc<dyn crate::core_host::storage::StorageBackend>>) {
    match previous {
        Some(backend) => {
            crate::core_host::storage::install(backend);
        }
        None => {
            crate::core_host::storage::clear();
        }
    }
}

#[tokio::test]
async fn a_storage_url_installs_the_driver_backed_store() {
    let _turn = SLOTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let previous = crate::core_host::agent::session_store::installed();
    let previous_backend = crate::core_host::storage::installed();
    install_for_url(Some("memory".into())).await.unwrap();

    let provider = crate::core_host::agent::session_store::installed().unwrap();
    assert!(
        provider
            .destination_key()
            .is_some_and(|key| key.starts_with("memory://")),
        "{:?}",
        provider.destination_key()
    );
    assert_eq!(
        crate::core_host::storage::installed().map(|b| b.driver()),
        Some("memory")
    );
    // Agents are kept apart in the shared backend.
    let alice = provider.for_agent("alice");
    alice
        .turn_states
        .put(&TurnState::started("t", "r", 8, "2026-01-01T00:00:00Z"))
        .unwrap();
    assert!(provider
        .for_agent("bob")
        .turn_states
        .get("t")
        .unwrap()
        .is_none());

    restore_backend(previous_backend);
    crate::core_host::agent::session_store::restore(previous);
}

#[tokio::test]
async fn no_url_keeps_the_classic_layout() {
    let _turn = SLOTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let previous = crate::core_host::agent::session_store::installed();
    let previous_backend = crate::core_host::storage::installed();
    install_for_url(None).await.unwrap();
    let provider = crate::core_host::agent::session_store::installed().unwrap();
    assert!(
        provider.workspace_dir().is_some(),
        "the file layout is installed"
    );
    assert!(crate::core_host::storage::installed().is_none());
    restore_backend(previous_backend);
    crate::core_host::agent::session_store::restore(previous);
}

#[tokio::test]
async fn an_unusable_url_fails_the_boot() {
    let _turn = SLOTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let previous = crate::core_host::agent::session_store::installed();
    let error = install_for_url(Some("ftp://nowhere".into()))
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("storage"), "{error:#}");
    crate::core_host::agent::session_store::restore(previous);
}

#[tokio::test]
async fn restoring_the_classic_layout_clears_a_previous_backend() {
    let _turn = SLOTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let previous = crate::core_host::agent::session_store::installed();
    let previous_backend = crate::core_host::storage::installed();
    install_for_url(Some("memory".into())).await.unwrap();
    assert!(crate::core_host::storage::installed().is_some());
    install_for_url(None).await.unwrap();
    assert!(crate::core_host::storage::installed().is_none());
    restore_backend(previous_backend);
    crate::core_host::agent::session_store::restore(previous);
}

#[tokio::test]
async fn the_host_reads_the_storage_url_from_the_environment() {
    let _turn = SLOTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let previous = crate::core_host::agent::session_store::installed();
    let previous_backend = crate::core_host::storage::installed();
    let var = crate::core_host::storage::STORAGE_URL_VAR;
    let old = std::env::var_os(var);

    std::env::set_var(var, " memory ");
    let booted = install_for_host().await;
    let driver = crate::core_host::storage::installed().map(|b| b.driver());

    std::env::set_var(var, "ftp://nowhere");
    let refused = install_for_host().await;

    match old {
        Some(value) => std::env::set_var(var, value),
        None => std::env::remove_var(var),
    }
    restore_backend(previous_backend);
    crate::core_host::agent::session_store::restore(previous);

    booted.unwrap();
    assert_eq!(driver, Some("memory"));
    assert!(refused.is_err(), "an unusable env URL fails the boot");
}
