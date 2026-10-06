//! Runtime smoke for the Sentry `before_send` filters that drop per-attempt
//! transient-upstream provider, backend_api, integrations, and updater
//! failures plus budget-exhausted user-state 400s (OPENHUMAN-TAURI-3M / 12 / 13).
//!
//! Unit tests in `crates/openhuman-core/src/core/observability.rs` exercise the pure filter
//! function. This integration test wires the actual `sentry::init` →
//! `before_send` → transport chain so we have proof the runtime path
//! behaves as designed: transient events are dropped, permanent events
//! and aggregate `all_exhausted` events still surface.

use openhuman_core::core::observability::{
    is_all_transient_provider_exhaustion_event, is_budget_event, is_session_expired_event,
    is_skill_install_user_fetch_failure, is_transient_backend_api_failure,
    is_transient_integrations_failure, is_transient_provider_http_failure,
    is_updater_transient_event,
};
use sentry::protocol::Event;
use std::collections::BTreeMap;
use std::sync::Arc;

fn event_with_tags(tags: &[(&str, &str)]) -> Event<'static> {
    let mut event = Event::default();
    let mut t: BTreeMap<String, String> = BTreeMap::new();
    for (k, v) in tags {
        t.insert((*k).to_string(), (*v).to_string());
    }
    event.tags = t;
    event
}

fn event_with_tags_and_message(tags: &[(&str, &str)], message: &str) -> Event<'static> {
    let mut event = event_with_tags(tags);
    event.message = Some(message.to_string());
    event
}

/// Drive an envelope-capturing Sentry client through a sequence of events
/// and return how many made it past `before_send`.
///
/// `sentry::init` mutates the process-global Sentry hub; Cargo runs integration
/// test functions in parallel threads by default, so two `count_captured` calls
/// would otherwise race on the global hub and one test's `capture_event` could
/// land in another test's transport. Serialize the critical section here rather
/// than imposing `--test-threads=1` on the whole binary.
fn count_captured(events: Vec<Event<'static>>) -> usize {
    static SENTRY_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = SENTRY_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let transport = sentry::test::TestTransport::new();
    let transport_for_factory = transport.clone();
    let options = sentry::ClientOptions {
        dsn: Some(
            "https://public@sentry.example.com/1"
                .parse()
                .expect("dsn parses"),
        ),
        // Same filter shape the real binary installs in main.rs.
        before_send: Some(Arc::new(|event| {
            if is_transient_provider_http_failure(&event)
                || is_all_transient_provider_exhaustion_event(&event)
                || is_transient_backend_api_failure(&event)
                || is_transient_integrations_failure(&event)
                || is_budget_event(&event)
                || is_updater_transient_event(&event)
                || is_skill_install_user_fetch_failure(&event)
                || is_session_expired_event(&event)
            {
                None
            } else {
                Some(event)
            }
        })),
        transport: Some(Arc::new(move |_opts: &sentry::ClientOptions| {
            transport_for_factory.clone() as Arc<dyn sentry::Transport>
        })),
        sample_rate: 1.0,
        ..sentry::ClientOptions::default()
    };
    let _sentry_guard = sentry::init(options);
    for event in events {
        sentry::capture_event(event);
    }
    sentry::Hub::current()
        .client()
        .map(|c| c.flush(Some(std::time::Duration::from_secs(2))));
    transport.fetch_and_clear_envelopes().len()
}

#[test]
fn before_send_chain_drops_transient_events_and_keeps_actionable_ones() {
    // Predicate coverage lives in `core/observability_*_tests.rs`; this pins the
    // `sentry::init` -> `before_send` -> transport wiring end to end.
    let dropped = vec![
        event_with_tags(&[
            ("domain", "llm_provider"),
            ("failure", "non_2xx"),
            ("status", "503"),
        ]),
        event_with_tags(&[
            ("domain", "backend_api"),
            ("failure", "non_2xx"),
            ("status", "502"),
        ]),
        event_with_tags_and_message(
            &[("domain", "integrations"), ("failure", "transport")],
            "GET /agent-integrations/tools failed: operation timed out",
        ),
    ];
    assert_eq!(
        count_captured(dropped),
        0,
        "transient failures must be filtered in before_send"
    );

    let kept = vec![
        event_with_tags(&[
            ("domain", "llm_provider"),
            ("failure", "non_2xx"),
            ("status", "401"),
        ]),
        event_with_tags(&[
            ("domain", "llm_provider"),
            ("failure", "all_exhausted"),
            ("attempts", "12"),
        ]),
    ];
    assert_eq!(
        count_captured(kept),
        2,
        "actionable failures must still reach Sentry"
    );
}
