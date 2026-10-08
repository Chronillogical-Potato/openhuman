//! The supply-chain scan gate shared by catalog and URL installs.
//!
//! A fetched `SKILL.md` whose scan blocks, or a fetch that fails, is fetched
//! and scanned once more. A document that still blocks is not installed
//! unless the user acknowledged its findings in the Skills UI; the caller gets
//! a [`SkillInstallOutcome::ScanBlocked`] instead.

use std::future::Future;

use serde::Serialize;
use tinyskills::{RegistryDocument, RegistryError, RegistryErrorKind, ScanCheck, Verdict};

use super::fetch::InstallWorkflowFromUrlOutcome;

/// Whether the user has reviewed a blocking scan and chosen to install anyway.
///
/// Only the Skills UI sets [`ScanAcknowledgement::ByUser`], through the
/// `acknowledge_scan_findings` JSON-RPC param. Agent tools always pass
/// [`ScanAcknowledgement::Absent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanAcknowledgement {
    /// Nobody acknowledged the findings: a blocking scan refuses the install.
    Absent,
    /// The user acknowledged the findings and asked to install anyway.
    ByUser,
}

impl ScanAcknowledgement {
    pub fn from_user_flag(acknowledged: bool) -> Self {
        if acknowledged {
            Self::ByUser
        } else {
            Self::Absent
        }
    }
}

/// One scan finding, in operator-facing language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScanFindingSummary {
    pub check: ScanCheck,
    pub verdict: Verdict,
    /// Where in the skill it fired, e.g. `the document body`.
    pub field: String,
    /// The finding as one readable line, without the offending value.
    pub message: String,
}

/// A document the scan blocked, returned instead of installing it.
#[derive(Debug, Clone, Serialize)]
pub struct ScanBlockedOutcome {
    /// The catalog entry id or URL the caller asked to install.
    pub target: String,
    /// The URL the document was fetched from, redacted.
    pub fetched_from: String,
    /// The install slug the document would have used.
    pub slug: String,
    pub findings: Vec<ScanFindingSummary>,
    pub message: String,
}

impl ScanBlockedOutcome {
    pub(crate) fn of(target: &str, document: &RegistryDocument) -> Self {
        let findings: Vec<ScanFindingSummary> = document
            .scan
            .findings
            .iter()
            .map(|finding| ScanFindingSummary {
                check: finding.check,
                verdict: finding.verdict,
                field: finding.field.label(),
                message: finding.message(),
            })
            .collect();
        let blocking: Vec<&str> = findings
            .iter()
            .filter(|finding| finding.verdict == Verdict::Block)
            .map(|finding| finding.message.as_str())
            .collect();
        Self {
            target: target.to_owned(),
            fetched_from: document.fetched_from.clone(),
            slug: document.document.slug.clone(),
            message: format!(
                "The security scan blocked this skill and it was not installed: {}.",
                blocking.join("; ")
            ),
            findings,
        }
    }
}

/// What an install did: installed the skill, or refused it on its scan.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SkillInstallOutcome {
    Installed(InstallWorkflowFromUrlOutcome),
    ScanBlocked(ScanBlockedOutcome),
}

impl SkillInstallOutcome {
    pub fn status(&self) -> &'static str {
        match self {
            Self::Installed(_) => "installed",
            Self::ScanBlocked(_) => "scan_blocked",
        }
    }

    /// The install, or `None` when the scan refused it.
    pub fn installed(self) -> Option<InstallWorkflowFromUrlOutcome> {
        match self {
            Self::Installed(installed) => Some(installed),
            Self::ScanBlocked(_) => None,
        }
    }
}

/// Whether a failed fetch is worth one more attempt. Refusals that depend
/// only on the request (an unknown id, an unsafe URL, a portal entry), an
/// oversized body and an upstream asking us to back off are not.
pub(crate) fn fetch_error_is_retryable(error: &RegistryError) -> bool {
    !matches!(
        error.kind(),
        RegistryErrorKind::NotFound
            | RegistryErrorKind::Ambiguous
            | RegistryErrorKind::UpstreamAmbiguous
            | RegistryErrorKind::NoDirectDownload
            | RegistryErrorKind::UnsafeUrl
            | RegistryErrorKind::UnknownRegistry
            | RegistryErrorKind::TooLarge
            | RegistryErrorKind::RateLimited
    )
}

/// Fetch and scan a document, retrying once when the scan blocks or the
/// fetch fails with a retryable error. An acknowledged install takes the
/// first document as is.
pub(crate) async fn fetch_scanned<F, Fut>(
    target: &str,
    acknowledgement: ScanAcknowledgement,
    mut fetch: F,
) -> Result<RegistryDocument, RegistryError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<RegistryDocument, RegistryError>>,
{
    match fetch().await {
        Ok(document) if !document.is_blocked() => return Ok(document),
        Ok(document) if acknowledgement == ScanAcknowledgement::ByUser => {
            tracing::warn!(
                install_target = %target,
                fetched_from = %document.fetched_from,
                findings = document.scan.findings.len(),
                "[skills] scan gate: blocked document installed on user acknowledgement"
            );
            return Ok(document);
        }
        Ok(document) => tracing::info!(
            install_target = %target,
            fetched_from = %document.fetched_from,
            findings = document.scan.findings.len(),
            "[skills] scan gate: scan blocked the document; fetching it once more"
        ),
        Err(error) if fetch_error_is_retryable(&error) => tracing::info!(
            install_target = %target,
            kind = error.kind().as_str(),
            error = %error,
            "[skills] scan gate: fetch failed; fetching it once more"
        ),
        Err(error) => return Err(error),
    }
    let retried = fetch().await;
    match &retried {
        Ok(document) => tracing::info!(
            install_target = %target,
            blocked = document.is_blocked(),
            "[skills] scan gate: retry fetched the document"
        ),
        Err(error) => tracing::info!(
            install_target = %target,
            kind = error.kind().as_str(),
            "[skills] scan gate: retry failed"
        ),
    }
    retried
}

/// Turn a scanned document into an install, or into a refusal when its scan
/// blocks and the user has not acknowledged it.
pub(crate) fn gate_install(
    target: &str,
    acknowledgement: ScanAcknowledgement,
    document: RegistryDocument,
    install: impl FnOnce(RegistryDocument) -> Result<InstallWorkflowFromUrlOutcome, String>,
) -> Result<SkillInstallOutcome, String> {
    if document.is_blocked() && acknowledgement == ScanAcknowledgement::Absent {
        let blocked = ScanBlockedOutcome::of(target, &document);
        tracing::warn!(
            install_target = %target,
            fetched_from = %blocked.fetched_from,
            slug = %blocked.slug,
            findings = blocked.findings.len(),
            "[skills] scan gate: refused install; scan still blocks after a retry"
        );
        return Ok(SkillInstallOutcome::ScanBlocked(blocked));
    }
    install(document).map(SkillInstallOutcome::Installed)
}

#[cfg(test)]
#[path = "scan_gate_tests.rs"]
mod tests;
