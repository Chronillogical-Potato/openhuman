use super::*;
use tinyagents_session::port::InMemorySessionStores;

#[tokio::test]
async fn a_scoped_provider_serves_only_its_own_task() {
    let provider: Arc<dyn SessionStoreProvider> = Arc::new(InMemorySessionStores::new());
    let inside = scope(provider.clone(), async {
        assert!(is_installed());
        for_agent("u1").map(|stores| stores.transcripts.destination_key())
    })
    .await;
    assert_eq!(
        inside,
        Some(provider.for_agent("u1").transcripts.destination_key())
    );
}

#[tokio::test]
async fn work_outside_an_agent_context_uses_the_default_agent() {
    let provider: Arc<dyn SessionStoreProvider> = Arc::new(InMemorySessionStores::new());
    let key = scope(provider.clone(), async {
        current().map(|stores| stores.transcripts.destination_key())
    })
    .await;
    // No agent context in a unit test: the shared default bucket.
    assert_eq!(
        key,
        Some(
            provider
                .for_agent(DEFAULT_AGENT)
                .transcripts
                .destination_key()
        )
    );
    assert!(current().is_none() || is_installed());
}

#[tokio::test]
async fn only_a_store_without_files_replaces_them() {
    assert!(!replaces_files(), "no store: the files are the record");
    let memory: Arc<dyn SessionStoreProvider> = Arc::new(InMemorySessionStores::new());
    assert!(scope(memory, async { replaces_files() }).await);
}

#[tokio::test]
async fn transcripts_come_from_the_store_only_when_one_is_in_effect() {
    assert!(transcripts_for("u1").is_none());
    let provider: Arc<dyn SessionStoreProvider> = Arc::new(InMemorySessionStores::new());
    let expected = provider.for_agent("u1").transcripts.destination_key();
    let found = scope(provider, async {
        transcripts_for("u1").map(|locator| locator.destination_key())
    })
    .await;
    assert_eq!(found, Some(expected));
}

#[tokio::test]
async fn without_a_store_transcripts_are_workspace_files() {
    let dir = std::path::Path::new("/nonexistent-openhuman-workspace");
    assert_eq!(
        transcripts_or_files("u1", dir).destination_key(),
        // tinyagents keys the file destination on the resolved `session_raw`
        // directory under the workspace (tinyagents#06ad95ea).
        Some(dir.join("session_raw").to_string_lossy().into_owned())
    );
    let provider: Arc<dyn SessionStoreProvider> = Arc::new(InMemorySessionStores::new());
    let expected = provider.for_agent("u1").transcripts.destination_key();
    let found = scope(provider, async {
        transcripts_or_files("u1", dir).destination_key()
    })
    .await;
    assert_eq!(found, expected);
}
