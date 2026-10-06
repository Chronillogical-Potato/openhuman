//! Detection and user-facing accessors for OpenHuman's own budget/quota limits —
//! inference credits and the SecurityPolicy per-hour action-budget cap. The
//! copy lives in the host copy table; the phrase matching in
//! `tinyinference_providers::billing`.

use crate::inference::failure_copy::{failure_copy, FailureClass};
use tinyinference_providers::{is_budget_message, BudgetMatch};

/// Whether an error string signals an exhausted inference budget, read with the
/// chat surface's [`BudgetMatch::Managed`] strictness: the managed no-credits
/// 400 ("Insufficient budget" / "Insufficient balance") plus the looser
/// top-up / out-of-credits wording, so the user gets the actionable budget copy
/// instead of the generic apology (#3088).
pub(crate) fn is_inference_budget_exceeded_error(message: &str) -> bool {
    is_budget_message(message, BudgetMatch::Managed)
}

pub(crate) fn inference_budget_exceeded_user_message() -> &'static str {
    // Keeps the literal "top up" / "credits" tokens (asserted by
    // `budget_exceeded_copy_mentions_top_up`) and the self-diagnosis path for
    // #3088. We guide, never auto-switch — the user's routing choice in
    // Settings is respected.
    failure_copy(FailureClass::BudgetExhausted).copy
}

pub(crate) fn generic_inference_error_user_message() -> &'static str {
    failure_copy(FailureClass::Inference).copy
}

/// Detect the SecurityPolicy global hourly action-budget signal
/// emitted by the built-in tools (`web_fetch`, `curl`, `http_request`,
/// `composio`, etc.) — see `crates/openhuman-core/src/security/
/// policy.rs::SecurityPolicy::is_rate_limited`.
///
/// We match the canonical English strings those tools emit. This is
/// load-bearing for issue #2364: before this check ran, any string
/// containing "rate limit" was misclassified as a provider 429 and
/// the user saw the generic "You're being rate-limited" copy, which
/// hides that the cap is OpenHuman's own per-hour safety budget,
/// not the upstream LLM provider.
pub(crate) fn is_action_budget_exhausted(err_lower: &str) -> bool {
    err_lower.contains("rate limit exceeded: action budget exhausted")
        || err_lower.contains("rate limit exceeded: too many actions in the last hour")
        || err_lower.contains("action blocked: rate limit exceeded")
}
