//! Classifying a managed-backend error by its stable `errorCode` (#870) —
//! the trusted fast path that runs before the substring ladder in
//! [`classify_inference_error`](super::classify::classify_inference_error).

use super::classify::ClassifiedError;
use super::retry::retry_after_hint;
use tinyinference_llm::failure::{is_malformed_tool_history_text, parse_retry_after_secs};

/// Classify a managed-backend error by its stable `errorCode` (#870).
///
/// Returns `Some` only when the flattened error string carries a *recognised*
/// backend `errorCode`. Because an `errorCode` is present **only** when the
/// error came through the managed backend, branching on it here lets us trust
/// the backend's verdict (operator faults route to the calm "temporarily
/// unavailable — we've been notified" copy, no user-blaming) instead of the
/// substring heuristics, which are tuned for the BYO / direct-provider path
/// (where no `errorCode` exists and "check your API key / model settings" is
/// the correct, user-actionable copy). See [`classify_inference_error`] (F2).
///
/// `None` falls through to the substring ladder, covering both the BYO path
/// (no code) and any future/unrecognised managed code we don't yet map.
pub(super) fn classify_by_backend_error_code(
    err: &str,
    provider: Option<String>,
    fallback_available: Option<bool>,
) -> Option<ClassifiedError> {
    use crate::inference::provider::{
        body_flags_malformed, extract_backend_error_code, is_managed_backend_envelope,
        BackendErrorCode,
    };

    // Managed-vs-BYO gate: an `errorCode` is only trustworthy on a
    // managed-backend envelope. A BYO / direct-provider body that merely
    // contains an `errorCode`-shaped field must fall through to the substring
    // ladder (CodeRabbit), keeping its user-actionable copy intact.
    if !is_managed_backend_envelope(err) {
        return None;
    }

    let code = extract_backend_error_code(err)?;

    // Verbose diagnostics on the new managed-code branch (per CLAUDE.md).
    // Low-cardinality only — the raw `err` may carry a provider payload / PII
    // and is logged at the caller, not here.
    log::debug!(
        "[chat-error][classify][errorCode] code={:?} provider={:?}",
        code,
        provider,
    );

    use super::classify::{classified, classified_plain, copy_params};
    use crate::inference::failure_copy::{failure_copy, FailureClass as C};

    let classified = match code {
        BackendErrorCode::RateLimited => {
            let retry_secs = parse_retry_after_secs(err);
            ClassifiedError {
                retry_after_ms: retry_secs.map(|s| s.saturating_mul(1000)),
                copy_params: copy_params(provider.as_deref(), retry_secs, None),
                ..classified(
                    C::ManagedRateLimited,
                    format!(
                        "{}{}",
                        failure_copy(C::ManagedRateLimited).copy,
                        retry_after_hint(retry_secs)
                    ),
                    provider,
                    fallback_available,
                )
            }
        }
        BackendErrorCode::UserInsufficientCredits => {
            classified_plain(C::ManagedBudgetExhausted, provider, None)
        }
        // Operator fault (our key/account/quota/5xx) OR operator registry /
        // routing misconfig — NOT user-actionable. Both route to the same
        // calm "we've been notified" copy; the backend already paged. We
        // deliberately DROP the "check your API key" (F4) and "pick a
        // different model" (F6) copy the BYO substring arms would emit.
        BackendErrorCode::UpstreamUnavailable | BackendErrorCode::ModelUnavailable => {
            classified_plain(C::ManagedUnavailable, provider, fallback_available)
        }
        BackendErrorCode::PayloadTooLarge => classified_plain(C::PayloadTooLarge, provider, None),
        BackendErrorCode::ContextLengthExceeded => {
            classified_plain(C::ContextOverflow, provider, None)
        }
        BackendErrorCode::BadRequest => {
            // Same code, three shapes. FIRST: a tool-ordering rejection
            // (`validateToolMessageOrdering` — an orphaned `role:'tool'` message
            // with no matching assistant `tool_call`) is *poisoned history*, not
            // a model/param problem. The de-poison guard in `run_task.rs` has
            // already evicted the offending warm session by the time this copy
            // is built, so the next turn cold-boots clean — tell the user
            // exactly that (and mark retryable, because resending now works).
            if is_malformed_tool_history_text(&err.to_lowercase()) {
                classified_plain(C::MalformedHistory, provider, None)
            // Else two shapes (B8/F8): a backend-flagged *malformed*
            // payload is a client bug (the request was built wrong — it pages
            // Sentry at the FE layer, gated elsewhere), while a plain
            // user-parameter rejection is a model/param mismatch the user can
            // fix. The copy differs: don't tell the user to abandon the thread
            // for a one-off malformation (only this turn failed).
            } else if body_flags_malformed(err) {
                classified_plain(C::ManagedMalformedRequest, provider, None)
            } else {
                classified_plain(C::ManagedRequestRejected, provider, None)
            }
        }
        // Backend already paged its own 500; the FE must not double-report
        // (gated in the Sentry classifier) and the user just retries.
        BackendErrorCode::InternalError => {
            classified_plain(C::ManagedInternal, provider, fallback_available)
        }
    };

    Some(classified)
}
