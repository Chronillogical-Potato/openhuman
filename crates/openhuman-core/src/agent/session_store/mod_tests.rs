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

/// The process-wide slot, exercised by one test so no other test in this
/// binary sees it set.
#[test]
fn installing_routes_every_agent_until_cleared() {
    assert!(!clear(), "nothing installed yet");
    let provider: Arc<dyn SessionStoreProvider> = Arc::new(InMemorySessionStores::new());
    install(provider.clone());
    assert!(is_installed());
    let stores = for_agent(DEFAULT_AGENT).expect("installed");
    assert_eq!(
        stores.transcripts.destination_key(),
        provider.for_agent(DEFAULT_AGENT).transcripts.destination_key()
    );
    assert!(clear());
    assert!(for_agent("u1").is_none());
}
