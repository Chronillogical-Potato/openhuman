//! Tests for the host time-zone lookup in [`super`].

use super::current_iana_timezone;

#[test]
fn current_iana_timezone_returns_non_empty_string() {
    // Behavioural contract: caller can always treat the return value
    // as a non-empty `timeZone` value. Empty would break Google
    // Calendar's validation.
    let tz = current_iana_timezone();
    assert!(!tz.is_empty(), "iana lookup must fall back to a value");
}
