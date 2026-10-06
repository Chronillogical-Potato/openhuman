use super::*;

#[tokio::test]
async fn run_events_rejects_missing_run_id() {
    let err = handle_run_events(Map::new()).await.unwrap_err();
    assert!(err.contains("invalid params"), "{err}");
}
