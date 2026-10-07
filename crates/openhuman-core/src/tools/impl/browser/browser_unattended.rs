//! Opt-in pass for allow-listed browser actions in trusted unattended turns.

use crate::agent::turn_origin::{self, AgentTurnOrigin, TrustedAutomationSource};
use crate::config::BrowserConfig;

pub(super) fn allowed_for(
    _origin: Option<&AgentTurnOrigin>,
    _browser: &BrowserConfig,
    _kind: &str,
) -> bool {
    let _ = TrustedAutomationSource::Cron;
    false
}

pub(super) fn allow_current(browser: &BrowserConfig, kind: &str, _digest_hex: &str) -> bool {
    allowed_for(turn_origin::current().as_ref(), browser, kind)
}

#[cfg(test)]
#[path = "browser_unattended_tests.rs"]
mod tests;
