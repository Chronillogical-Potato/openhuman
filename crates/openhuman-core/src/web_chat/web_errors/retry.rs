//! Formatting the "retry after" hint for users, and telling a retryable 429
//! apart from a non-retryable business one. Parsing the hint out of an error
//! string is `tinyinference_llm::failure::parse_retry_after_secs`.

/// Format the retry-after hint as a short user-friendly suffix
/// (`" Try again in 30 seconds."`). Returns an empty string when no
/// hint is available so callers can `format!("{summary}{hint}")`
/// without branching on `Option`.
pub(crate) fn retry_after_hint(secs: Option<u64>) -> String {
    match secs {
        Some(0) => " You can retry immediately.".to_string(),
        Some(1) => " Try again in 1 second.".to_string(),
        Some(n) if n < 90 => format!(" Try again in {n} seconds."),
        Some(n) => {
            // Round UP — never tell the user to retry sooner than
            // the upstream actually allows. 90–119s used to render
            // as "about 1 minutes" both because of integer flooring
            // and missing singular/plural handling (CodeRabbit
            // review on #2371).
            let mins = (n / 60) + u64::from(n % 60 != 0);
            let unit = if mins == 1 { "minute" } else { "minutes" };
            format!(" Try again in about {mins} {unit}.")
        }
        None => String::new(),
    }
}

/// Whether a flattened, already-lowercased 429 message is a business limit
/// (plan, balance, quota, package) that a retry cannot clear.
///
/// The reliable provider classifies 429s into retryable vs non-retryable from
/// the typed error, but that `anyhow::Error` is collapsed to a `String` at the
/// native-bus boundary before reaching this layer. The verdict is the one
/// `classify_provider_failure` reaches; the marker list and the Z.AI business
/// codes (1113 / 1311) live in `tinyinference_llm::failure`, not here.
///
/// Caller passes the already-lowercased error string to avoid double
/// allocation.
pub(crate) fn is_non_retryable_rate_limit_text(lower: &str) -> bool {
    tinyinference_llm::failure::contains_business_limit(lower)
}
