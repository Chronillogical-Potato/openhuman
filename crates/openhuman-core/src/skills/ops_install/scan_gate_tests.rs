use super::*;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[test]
fn only_an_explicit_user_flag_acknowledges_findings() {
    assert_eq!(
        ScanAcknowledgement::from_user_flag(true),
        ScanAcknowledgement::ByUser
    );
    assert_eq!(
        ScanAcknowledgement::from_user_flag(false),
        ScanAcknowledgement::Absent
    );
}

#[test]
fn outages_and_bad_documents_are_retried_but_request_refusals_are_not() {
    for retryable in [
        RegistryError::Unavailable { status: 503 },
        RegistryError::Timeout {
            operation: "document",
            budget: std::time::Duration::from_secs(1),
        },
    ] {
        assert!(fetch_error_is_retryable(&retryable), "{retryable}");
    }
    for refused in [
        RegistryError::NotFound {
            id: "x".into(),
            closest: Vec::new(),
        },
        RegistryError::NoDirectDownload {
            name: "x".into(),
            source_url: None,
        },
        RegistryError::RateLimited { retry_after: None },
    ] {
        assert!(!fetch_error_is_retryable(&refused), "{refused}");
    }
}

async fn run_failing(error: fn() -> RegistryError) -> (usize, RegistryError) {
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&calls);
    let result = fetch_scanned("entry", ScanAcknowledgement::Absent, move || {
        counted.fetch_add(1, Ordering::SeqCst);
        let error = error();
        async move { Err::<RegistryDocument, _>(error) }
    })
    .await;
    (
        calls.load(Ordering::SeqCst),
        result.expect_err("every attempt fails"),
    )
}

#[tokio::test]
async fn a_failed_fetch_is_attempted_exactly_twice() {
    let (calls, error) = run_failing(|| RegistryError::Unavailable { status: 503 }).await;
    assert_eq!(calls, 2, "one retry after the failure");
    assert_eq!(error.kind(), RegistryErrorKind::Unavailable);
}

#[tokio::test]
async fn a_refused_request_is_not_retried() {
    let (calls, error) = run_failing(|| RegistryError::NotFound {
        id: "x".into(),
        closest: Vec::new(),
    })
    .await;
    assert_eq!(calls, 1);
    assert_eq!(error.kind(), RegistryErrorKind::NotFound);
}

#[test]
fn outcomes_serialize_with_a_status_tag() {
    let installed = SkillInstallOutcome::Installed(InstallWorkflowFromUrlOutcome {
        url: "u".into(),
        stdout: "Installed to /x".into(),
        stderr: String::new(),
        new_skills: vec!["x".into()],
    });
    let value = serde_json::to_value(&installed).unwrap();
    assert_eq!(value["status"], "installed");
    assert_eq!(value["new_skills"][0], "x");
    assert_eq!(installed.status(), "installed");

    let blocked = SkillInstallOutcome::ScanBlocked(ScanBlockedOutcome {
        target: "x".into(),
        fetched_from: "https://example.com/SKILL.md".into(),
        slug: "x".into(),
        findings: vec![ScanFindingSummary {
            check: ScanCheck::InvisibleCodePoints,
            verdict: Verdict::Block,
            field: "the document body".into(),
            message: "an invisible character in the document body".into(),
        }],
        message: "blocked".into(),
    });
    let value = serde_json::to_value(&blocked).unwrap();
    assert_eq!(value["status"], "scan_blocked");
    assert_eq!(value["findings"][0]["check"], "invisible_code_points");
    assert_eq!(value["findings"][0]["verdict"], "block");
    assert_eq!(blocked.status(), "scan_blocked");
}
