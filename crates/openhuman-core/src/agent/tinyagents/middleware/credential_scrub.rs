//! Host wiring for the harness `CredentialScrubMiddleware`: the scrub itself
//! lives upstream; the only host policy is the browser task's confirmation
//! token, which is minted by the host and must survive the scrubber.

use tinyagents_harness::middleware::{
    redaction_notice, scrub_with_notice, CredentialScrubMiddleware, ToolScrubber,
    REDACTION_PLACEHOLDER,
};

/// The browser task's `pending.token` is a host-minted, one-time confirmation
/// handle, not a credential from page content. Protect only that field in a
/// `NeedsConfirmation` result; every other string still crosses the ordinary
/// credential scrubber, including page text and action input.
///
/// The token is removed before scrubbing and put back afterwards, so it is
/// never seen by (or counted against) the scrubber.
fn scrub_with_notice_for_tool(tool_name: &str, content: &str) -> Option<(String, usize)> {
    if tool_name != "browser" {
        return scrub_with_notice(content);
    }
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(content) else {
        return scrub_with_notice(content);
    };
    if value["status"] != "NeedsConfirmation" {
        return scrub_with_notice(content);
    }
    let Some(token) = value["pending"]["token"].as_str().map(str::to_owned) else {
        return scrub_with_notice(content);
    };
    if token.len() != 36 || uuid::Uuid::parse_str(&token).is_err() {
        return scrub_with_notice(content);
    }
    // Take the token out of the document instead of masking it with a dummy
    // value: the scrubber redacts secret-looking `token` values of any
    // length, so a UUID-shaped placeholder is redacted and counted too.
    if let Some(pending) = value["pending"].as_object_mut() {
        pending.remove("token");
    }
    let protected = match serde_json::to_string(&value) {
        Ok(protected) => protected,
        Err(_) => return scrub_with_notice(content),
    };
    let scrubbed = tinyinference_core::sanitize::scrub_credentials(&protected);
    if scrubbed == protected {
        return None;
    }
    let redactions = scrubbed
        .matches(REDACTION_PLACEHOLDER)
        .count()
        .saturating_sub(protected.matches(REDACTION_PLACEHOLDER).count());
    let mut result: serde_json::Value = match serde_json::from_str(&scrubbed) {
        Ok(result) => result,
        Err(_) => return scrub_with_notice(content),
    };
    result["pending"]["token"] = serde_json::Value::String(token);
    Some((
        format!("{}\n\n{}", result, redaction_notice(redactions)),
        redactions,
    ))
}

/// The credential scrub wrap the turn harness installs innermost.
pub(crate) fn credential_scrub_middleware() -> CredentialScrubMiddleware {
    let scrubber: ToolScrubber = std::sync::Arc::new(scrub_with_notice_for_tool);
    CredentialScrubMiddleware::with_scrubber(scrubber)
}

#[cfg(test)]
#[path = "credential_scrub_tests.rs"]
mod tests;
