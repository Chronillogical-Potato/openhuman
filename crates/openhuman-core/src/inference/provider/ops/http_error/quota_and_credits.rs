//! Budget, quota, and per-request rate-cap classification — the deterministic
//! "third-party account/plan is out of runway" family of provider errors.

/// Whether a provider non-2xx response is a deterministic budget-exhausted
/// user-state error that should be demoted from Sentry to an info log.
pub fn is_budget_exhausted_http_400(status: reqwest::StatusCode, body: &str) -> bool {
    status == reqwest::StatusCode::BAD_REQUEST
        && tinyinference_providers::is_budget_exhausted_message(body)
}

pub fn log_budget_exhausted_http_400(
    operation: &str,
    provider: &str,
    model: Option<&str>,
    status: reqwest::StatusCode,
) {
    tracing::info!(
        domain = "llm_provider",
        operation = operation,
        provider = provider,
        model = model.unwrap_or(""),
        status = status.as_u16(),
        failure = "non_2xx",
        kind = "budget",
        "[llm_provider] {operation} budget-exhausted 400 — not reporting to Sentry"
    );
}

/// Whether a provider non-2xx response is a deterministic
/// **insufficient-credits** user-state error — the BYO provider account
/// (e.g. OpenRouter) lacks the balance to satisfy the request.
///
/// This is the *residual* case once the request already caps `max_tokens`
/// (so the provider's pre-flight is priced against a realistic output budget
/// rather than the model's full window — see
/// [`crate::inference::provider::ChatRequest::max_tokens`]): a 402
/// that still arrives means the user's own third-party account is genuinely
/// out of credit, a billing state OpenHuman has no lever over. Demote from
/// Sentry to an info log rather than page once per retry
/// (TAURI-RUST-C62: 12k events from a single low-balance user).
///
/// Gated on the 402 status **and** a credit/payment phrase so an unrelated
/// 402 is not swallowed. The phrase list is covered by a verbatim-body test
/// so a provider wording drift fails CI instead of silently leaking events.
pub fn is_provider_insufficient_credits_402(status: reqwest::StatusCode, body: &str) -> bool {
    status == reqwest::StatusCode::PAYMENT_REQUIRED
        && tinyinference_llm::failure::body_indicates_insufficient_credits(body)
}

pub fn log_provider_insufficient_credits_402(
    operation: &str,
    provider: &str,
    model: Option<&str>,
    status: reqwest::StatusCode,
) {
    tracing::info!(
        domain = "llm_provider",
        operation = operation,
        provider = provider,
        model = model.unwrap_or(""),
        status = status.as_u16(),
        failure = "non_2xx",
        kind = "insufficient_credits",
        "[llm_provider] {operation} provider insufficient-credits 402 — BYO account out of \
         balance (no local lever), not reporting to Sentry"
    );
}

pub fn log_provider_quota_exhausted(
    operation: &str,
    provider: &str,
    model: Option<&str>,
    status: reqwest::StatusCode,
) {
    tracing::info!(
        domain = "llm_provider",
        operation = operation,
        provider = provider,
        model = model.unwrap_or(""),
        status = status.as_u16(),
        failure = "non_2xx",
        kind = "quota_exhausted",
        "[llm_provider] {operation} provider monthly-quota exhausted — third-party plan limit \
         reached (no local lever), not reporting to Sentry"
    );
}
