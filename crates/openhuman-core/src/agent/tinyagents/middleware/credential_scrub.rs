//! [`CredentialScrubMiddleware`]: scrub credential-shaped secrets out of every
//! tool result before it leaves the tool boundary.

use async_trait::async_trait;

use tinyagents_harness::context::RunContext;
use tinyagents_harness::error::Result as TaResult;
use tinyagents_harness::middleware::{MiddlewareToolOutcome, ToolHandler, ToolMiddleware};
use tinyinference_llm::tool::ToolCall as TaToolCall;

/// `wrap_tool`: scrub credential-shaped secrets out of every tool result before
/// it leaves the tool boundary (issue #4453). The legacy engine ran
/// `scrub_credentials` over **every** tool output before it entered model
/// context (`engine/tools.rs`); the tinyagents path dropped that call site, so
/// secrets in tool output (env dumps, config reads, API responses, shell output)
/// reached model context, on-disk `session_raw` transcripts, worker-thread
/// mirrors, and the tool-outcome capture sink — violating "Never log secrets or
/// full PII".
///
/// Installed as the **innermost** tool wrap (pushed last), so it observes the
/// RAW tool result first and scrubs it before any outer wrap, the `after_tool`
/// chain (summarization/caps in [`ToolOutputMiddleware`]), the transcript push,
/// or the [`ToolOutcomeCaptureMiddleware`] sink can see the unredacted content.
/// Scrubbing here — rather than inside tool dispatch — covers the
/// parent chat path, sub-agent paths, the persisted transcript, and
/// `ToolCallOutcome` records by construction, since every path runs the same
/// `assemble_turn_harness` seam.
/// The placeholder `scrub_credentials` emits once per redacted value. Counted
/// to tell the model *how many* values went, which is the difference between
/// "something was withheld" and a number it can relay.
const REDACTION_PLACEHOLDER: &str = "*[REDACTED]";

/// Appended to a scrubbed tool result so the **model** learns what the log
/// already knew.
///
/// Without it the model receives a result that silently differs from what the
/// tool returned: it cannot find the content it was asked for, re-runs the same
/// call, gets an identically scrubbed result, and never converges — until the
/// successful-repeat tracker halts the run and the user is told "Incomplete"
/// with no reason (#6416). The redaction itself is correct and unchanged; only
/// its silence was the defect.
///
/// The "do not retry" clause is load-bearing: a retry is guaranteed to be
/// scrubbed identically, so it is the one action that cannot help.
///
/// Deliberately contains no `<keyword>: <value>` shape, so it cannot match
/// `SENSITIVE_KV_REGEX` and scrub itself on a second pass — pinned by
/// `the_notice_does_not_scrub_itself`.
fn redaction_notice(count: usize) -> String {
    format!(
        "[credential_scrub] {count} value(s) in this result were redacted as credentials. \
         Re-running this tool returns the same redaction, so do not retry — tell the user \
         which values were withheld and that they can view them directly in the source app."
    )
}

/// Scrub `content`, returning the replacement text **and** how many values
/// went — or `None` when nothing was credential-shaped.
///
/// Split out of `wrap_tool` so the decision and the composed result are
/// directly testable. Exercising this is the difference between proving the
/// notice text is well-formed and proving a scrubbed result actually carries
/// it; only `replace_tool_result_text` plumbing stays untested.
fn scrub_with_notice(content: &str) -> Option<(String, usize)> {
    let scrubbed = crate::agent::harness::credentials::scrub_credentials(content);
    if scrubbed == content {
        return None;
    }
    // Count what this pass removed, not what the text already carried — a
    // result may legitimately contain the placeholder already.
    let redactions = scrubbed
        .matches(REDACTION_PLACEHOLDER)
        .count()
        .saturating_sub(content.matches(REDACTION_PLACEHOLDER).count());
    Some((
        format!("{scrubbed}\n\n{}", redaction_notice(redactions)),
        redactions,
    ))
}

/// The browser task's `pending.token` is a host-minted, one-time confirmation
/// handle, not a credential from page content. Protect only that field in a
/// `NeedsConfirmation` result; every other string still crosses the ordinary
/// credential scrubber, including page text and action input.
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
    value["pending"]["token"] = serde_json::Value::String("x".into());
    let protected = match serde_json::to_string(&value) {
        Ok(protected) => protected,
        Err(_) => return scrub_with_notice(content),
    };
    let scrubbed = crate::agent::harness::credentials::scrub_credentials(&protected);
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

pub(crate) struct CredentialScrubMiddleware;

impl CredentialScrubMiddleware {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ToolMiddleware<(), crate::agent::tinyagents::host::OpenHumanRunContext>
    for CredentialScrubMiddleware
{
    fn name(&self) -> &str {
        "credential_scrub"
    }

    async fn wrap_tool(
        &self,
        ctx: &mut RunContext<crate::agent::tinyagents::host::OpenHumanRunContext>,
        state: &(),
        call: TaToolCall,
        next: ToolHandler<'_, (), crate::agent::tinyagents::host::OpenHumanRunContext>,
    ) -> TaResult<MiddlewareToolOutcome> {
        let tool_name = call.name.clone();
        let outcome = next.run(ctx, state, call).await?;
        // `MiddlewareToolOutcome` is `#[non_exhaustive]`; today it only carries a
        // `Result`, but match rather than irrefutable-let so a future variant
        // fails loud instead of silently bypassing scrubbing.
        let mut result = match outcome {
            MiddlewareToolOutcome::Result(result) => result,
            other => return Ok(other),
        };

        let content = crate::agent::tinyagents::middleware::tool_result_text(&result);
        if let Some((annotated, redactions)) = scrub_with_notice_for_tool(&tool_name, &content) {
            tracing::warn!(
                tool = %tool_name,
                redactions,
                "[tinyagents::mw] credential_scrub redacted secret(s) from tool result content"
            );
            // The notice goes to the model, in the result itself. The warning
            // above goes to the log, where no model will ever read it — which
            // was the whole defect (#6416).
            crate::agent::tinyagents::middleware::replace_tool_result_text(&mut result, annotated);
        }

        Ok(MiddlewareToolOutcome::Result(result))
    }
}

#[cfg(test)]
#[path = "credential_scrub_tests.rs"]
mod tests;
