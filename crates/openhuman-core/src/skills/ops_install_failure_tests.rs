//! Install-time failure modes for the SKILL.md fetch (#6409).
//!
//! Split out of `ops_create_and_url_tests.rs`, which reached the 750-line
//! layout cap. These cases share a shape the happy-path install tests do not:
//! each drives `install_workflow_from_url_with_home` against a wiremock host
//! that answers with a *failure*, and asserts on the KIND of error produced —
//! a throttled host must be distinguishable from a missing skill, or the UI
//! cannot tell the user whether to wait or to give up.

use super::*;

/// #6409: a throttled host and an unreachable one must not produce the same
/// error. Both used to be `fetch failed: … returned status N` / a transport
/// string with nothing distinguishing them, so the UI could not tell the user
/// whether to wait or to give up.
///
/// Asserting "install fails" would pass on the code this replaces, so each case
/// asserts the *kind* it produces and, for 429, that the host's own
/// `Retry-After` survives into the message.
#[tokio::test]
async fn a_throttled_host_reports_rate_limiting_with_its_retry_after() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/SKILL.md"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "42"))
        .mount(&server)
        .await;

    let workspace = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let error = install_workflow_from_url_with_home(
        workspace.path(),
        InstallWorkflowFromUrlParams {
            url: format!("{}/SKILL.md", server.uri()),
            timeout_secs: Some(5),
        },
        Some(home.path()),
        true,
    )
    .await
    .expect_err("a 429 must fail the install");

    assert!(
        error.starts_with(crate::skills::ops_install::RATE_LIMITED_ERROR_PREFIX),
        "a throttled host must be reported as rate limiting, got: {error}"
    );
    assert!(
        error.contains("42s"),
        "the host's own Retry-After must survive into the message, got: {error}"
    );
}

/// A 429 without a `Retry-After` is still rate limiting — the caller just has
/// no delay to honour. It must not fall through to the generic status string.
#[tokio::test]
async fn a_throttled_host_without_retry_after_still_reports_rate_limiting() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/SKILL.md"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;

    let workspace = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let error = install_workflow_from_url_with_home(
        workspace.path(),
        InstallWorkflowFromUrlParams {
            url: format!("{}/SKILL.md", server.uri()),
            timeout_secs: Some(5),
        },
        Some(home.path()),
        true,
    )
    .await
    .expect_err("a 429 must fail the install");

    assert!(
        error.starts_with(crate::skills::ops_install::RATE_LIMITED_ERROR_PREFIX),
        "a 429 with no Retry-After is still rate limiting, got: {error}"
    );
}

/// The other direction: a 404 is a missing skill, not throttling, and must keep
/// the generic status wording. Without this the rate-limit branch could widen
/// to every non-2xx and the distinction would be lost again.
#[tokio::test]
async fn a_missing_skill_is_not_reported_as_rate_limiting() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/SKILL.md"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let workspace = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let error = install_workflow_from_url_with_home(
        workspace.path(),
        InstallWorkflowFromUrlParams {
            url: format!("{}/SKILL.md", server.uri()),
            timeout_secs: Some(5),
        },
        Some(home.path()),
        true,
    )
    .await
    .expect_err("a 404 must fail the install");

    assert!(
        !error.starts_with(crate::skills::ops_install::RATE_LIMITED_ERROR_PREFIX),
        "a 404 is a missing skill, not throttling, got: {error}"
    );
    assert!(error.contains("404"), "got: {error}");
}
