use super::*;
use std::collections::BTreeMap;
use tinyagents_harness::observability::trace_export::{SpanKind, SpanStatus};

#[tokio::test]
async fn external_backend_is_skipped_before_session_lookup() {
    let mut config = Config::default();
    config.api_url = Some("https://external.example.invalid/openai/v1".into());
    let spans = [TraceSpan {
        trace_id: "thread-one:turn-one".into(),
        span_id: "root".into(),
        parent_span_id: None,
        name: "agent.turn".into(),
        kind: SpanKind::Turn,
        start_unix_ms: 1_000,
        end_unix_ms: Some(2_000),
        status: SpanStatus::Ok,
        attributes: BTreeMap::new(),
        input: None,
        output: None,
    }];
    assert!(push_spans(&config, &spans).await.is_ok());
}
