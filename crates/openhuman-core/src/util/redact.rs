//! PII redaction for log output.
//!
//! Per `CLAUDE.md`: "Never log secrets or full PII." Source ids, entity ids and
//! content paths can embed full email addresses, so any log line printing one
//! has to redact it first.
//!
//! # Why this is OpenHuman's and not the memory crate's
//!
/// Redact a URL's userinfo (username/password) for safe logging.
///
/// Parses `raw` (falling back to an assumed `http://` scheme when it has
/// none), replaces any embedded username/password with the literal
/// `"redacted"`, and strips a trailing slash. Returns `raw` trimmed and
/// unchanged when it cannot be parsed as a URL at all.
#[must_use]
pub fn redact_url_for_log(raw: &str) -> String {
    let trimmed = raw.trim();
    let parsed =
        url::Url::parse(trimmed).or_else(|_| url::Url::parse(&format!("http://{trimmed}")));
    let Ok(mut parsed) = parsed else {
        return trimmed.to_string();
    };
    if !parsed.username().is_empty() {
        let _ = parsed.set_username("redacted");
    }
    if parsed.password().is_some() {
        let _ = parsed.set_password(Some("redacted"));
    }
    parsed.set_query(None);
    parsed.set_fragment(None);
    parsed.to_string().trim_end_matches('/').to_string()
}

