//! Opt-in pass for allow-listed browser actions in trusted unattended turns.
//!
//! Every consequential browser action asks the forced host approval gate,
//! which parks only for a routable WebChat turn and denies everything else.
//! A cron job, background job or approval-free workflow has nobody to answer,
//! so it could not even follow a "next page" link. `[browser]
//! unattended_actions` names the gated kinds such a turn may take without
//! asking; every other origin keeps the forced gate exactly as it was.

use crate::agent::turn_origin::{self, AgentTurnOrigin, TrustedAutomationSource};
use crate::config::BrowserConfig;

/// A turn the user authorized ahead of time and nobody is watching live.
/// A workflow that asked for review (`require_approval`) is excluded.
fn is_unattended(origin: &AgentTurnOrigin) -> bool {
    matches!(
        origin,
        AgentTurnOrigin::TrustedAutomation {
            source: TrustedAutomationSource::Cron
                | TrustedAutomationSource::Background
                | TrustedAutomationSource::Workflow {
                    require_approval: false
                },
            ..
        }
    )
}

/// Whether a turn with `origin` may take `kind` without the approval gate.
pub(super) fn allowed_for(
    origin: Option<&AgentTurnOrigin>,
    browser: &BrowserConfig,
    kind: &str,
) -> bool {
    origin.is_some_and(is_unattended) && browser.allows_unattended(kind)
}

/// Decide for the current turn, logging an allowed action by kind and digest
/// only: no selector, typed value or page content reaches the log.
pub(super) fn allow_current(browser: &BrowserConfig, kind: &str, digest_hex: &str) -> bool {
    let _ = digest_hex;
    allowed_for(turn_origin::current().as_ref(), browser, kind)
}

#[cfg(test)]
#[path = "browser_unattended_tests.rs"]
mod tests;
