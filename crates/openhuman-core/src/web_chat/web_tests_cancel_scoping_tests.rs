//! Request-scoped web-chat cancellation (#4760): a stale scoped cancel for a
//! superseded request must not tear down the newer in-flight turn.

use crate::web_chat::cancel_should_target;

#[test]
fn unscoped_cancel_always_targets_the_in_flight_turn() {
    // No request_id => "stop whatever is running on this thread" (Stop button /
    // session teardown).
    assert!(cancel_should_target(None, "req-A"));
    assert!(cancel_should_target(None, "req-B"));
}

#[test]
fn scoped_cancel_fires_for_its_own_request() {
    assert!(cancel_should_target(Some("req-A"), "req-A"));
}

#[test]
fn stale_scoped_cancel_does_not_kill_a_newer_turn() {
    // Request A timed out client-side and B is now in flight: A's late scoped
    // cancel must NOT tear down B.
    assert!(!cancel_should_target(Some("req-A"), "req-B"));
    assert!(!cancel_should_target(
        Some("old-request"),
        "current-request"
    ));
}
