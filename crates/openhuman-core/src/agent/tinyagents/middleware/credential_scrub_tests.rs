//! #6416: the redaction must reach the model, not only the log.
//!
//! The scrubber's behaviour is deliberately NOT relaxed here — a signed
//! magic-link token in an email is a credential, and letting it through would
//! re-open #4453 for the commonest credential shape in mail. What changes is
//! that a scrubbed result now says so, so the agent stops re-fetching content
//! that will be redacted identically every time.
//!
//! The first test is the one that matters: it proves the redaction still
//! happens. A notice that appeared while the scrubbing silently weakened would
//! be the worst outcome this change could have.

use super::*;
use crate::agent::harness::credentials::scrub_credentials;

/// A magic-link token — exactly the shape QA hit in #6416 — must still be
/// removed. This is the security half of the contract and comes first.
#[test]
fn a_link_token_in_email_content_is_still_redacted() {
    let email = "Click https://acme.example/verify?token=aB3dE5fG7hJ9kL1mN3pQ to sign in.";
    let scrubbed = scrub_credentials(email);

    assert!(
        !scrubbed.contains("aB3dE5fG7hJ9kL1mN3pQ"),
        "the token body must not survive scrubbing: {scrubbed}"
    );
    assert!(
        scrubbed.contains(REDACTION_PLACEHOLDER),
        "a redaction must leave its placeholder: {scrubbed}"
    );
}

/// The notice must not match the scrubber's own patterns, or a second pass
/// would redact the explanation and the model would be told even less than
/// before. Verified rather than reasoned about: `credential` is followed by
/// `]` and a space here, never by `["']?\s*[:=]`.
#[test]
fn the_notice_does_not_scrub_itself() {
    let notice = redaction_notice(2);
    assert_eq!(
        scrub_credentials(&notice),
        notice,
        "the notice must survive the scrubber unchanged, or it would redact its own explanation"
    );
}

/// Scrubbing already-scrubbed content must be a no-op, which is what makes the
/// middleware idempotent: the `scrubbed != content` guard is false on a second
/// pass, so no second notice is appended and the count is not restated.
#[test]
fn scrubbing_is_idempotent_so_a_second_pass_adds_no_second_notice() {
    let email = "Click https://acme.example/verify?token=aB3dE5fG7hJ9kL1mN3pQ to sign in.";
    let once = scrub_credentials(email);
    let twice = scrub_credentials(&once);
    assert_eq!(twice, once, "scrub must be stable on its own output");

    // The value the middleware would hand on: scrubbed body plus the notice.
    let annotated = format!("{once}\n\n{}", redaction_notice(1));
    assert_eq!(
        scrub_credentials(&annotated),
        annotated,
        "a result already carrying the notice must pass through untouched"
    );
}

/// The count is what the model relays to the user, so it must describe this
/// pass rather than every placeholder in the text.
#[test]
fn the_notice_counts_only_what_this_pass_removed() {
    let two = "api_key=aB3dE5fG7hJ9kL1mN3pQ and token=zX9yW8vU7tS6rQ5pO4nM";
    let scrubbed = scrub_credentials(two);
    let removed = scrubbed.matches(REDACTION_PLACEHOLDER).count()
        - two.matches(REDACTION_PLACEHOLDER).count();
    assert_eq!(removed, 2, "both values were redacted: {scrubbed}");

    assert!(redaction_notice(removed).contains("2 value(s)"));
    assert!(
        redaction_notice(removed).contains("do not retry"),
        "the notice must tell the model retrying cannot help — that loop is the defect"
    );
}

/// Ordinary email content with nothing credential-shaped must be untouched, so
/// the notice never appears on a result that lost nothing.
#[test]
fn unremarkable_content_is_left_alone_and_gets_no_notice() {
    let email = "Lunch at 12:30 tomorrow? The meeting room is booked until 2pm.";
    assert_eq!(
        scrub_credentials(email),
        email,
        "nothing here is credential-shaped; a notice would be a lie"
    );
}

/// The behaviour this change exists for: a scrubbed result **carries** the
/// notice. The tests above prove the notice text is well-formed and that the
/// redaction still happens; this proves the two are actually joined, which is
/// the part a caller sees.
#[test]
fn a_scrubbed_result_carries_the_notice_and_the_count() {
    let email = "Click https://acme.example/verify?token=aB3dE5fG7hJ9kL1mN3pQ to sign in.";
    let (annotated, redactions) =
        scrub_with_notice(email).expect("credential-shaped content must be scrubbed");

    assert_eq!(redactions, 1);
    assert!(
        !annotated.contains("aB3dE5fG7hJ9kL1mN3pQ"),
        "the token must not survive: {annotated}"
    );
    assert!(
        annotated.contains("[credential_scrub]"),
        "the model must be told the result was altered: {annotated}"
    );
    assert!(
        annotated.contains("do not retry"),
        "the model must be told retrying cannot help: {annotated}"
    );
}

/// And a result that lost nothing must be left exactly alone — no notice, and
/// no rewrite of the tool's own output.
#[test]
fn an_unscrubbed_result_is_not_annotated_at_all() {
    assert!(
        scrub_with_notice("Lunch at 12:30 tomorrow? Room booked until 2pm.").is_none(),
        "nothing was redacted, so the result must pass through untouched"
    );
}

#[test]
fn browser_confirmation_token_survives_without_exempting_page_credentials() {
    let pending = serde_json::json!({
        "status": "NeedsConfirmation",
        "pending": {
            "action": {"action": "click", "target": {"kind": "ref", "value": "e33"}},
            "token": "00000000-0000-4000-8000-000000000123"
        },
        "page": {
            "api_key": "sk-abcdefghijklmnopqrstuvwxyz123456",
            "text": "token=page-secret-value"
        }
    });
    let content = pending.to_string();
    let (scrubbed, count) =
        scrub_with_notice_for_tool("browser", &content).expect("page credential is still redacted");
    assert_eq!(count, 2);
    assert!(scrubbed.contains("00000000-0000-4000-8000-000000000123"));
    assert!(!scrubbed.contains("abcdefghijklmnopqrstuvwxyz123456"));
    assert!(!scrubbed.contains("page-secret-value"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(scrubbed.split("\n\n").next().unwrap()).unwrap()
            ["pending"]["token"],
        pending["pending"]["token"]
    );

    let token_only = serde_json::json!({
        "status": "NeedsConfirmation",
        "pending": {"token": "00000000-0000-4000-8000-000000000123"}
    });
    assert!(scrub_with_notice_for_tool("browser", &token_only.to_string()).is_none());
}
