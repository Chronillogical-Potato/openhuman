//! Parameter redaction for RPC log lines.
//!
//! [`redact_params_for_log`] is what `core::dispatch` calls for its
//! `[rpc:dispatch] enter` trace line. It strips a set of sensitive keys —
//! `api_key`, `apikey`, `token`, `access_token`, `refresh_token`,
//! `authorization`, `password`, `secret`, `client_secret` — from JSON objects
//! (recursing into nested objects/arrays) before a log line is emitted, since
//! `serde_json::Value` params routinely embed provider credentials.
//!
//! This is the log-side counterpart of `core::log_redaction::scrub_secrets`,
//! which pattern-matches secrets embedded in free-text error strings; this
//! module instead redacts by JSON *key name* in structured params.

use serde_json::Value;

/// Redacts sensitive keys from a JSON parameters object before logging.
///
/// This is used to prevent accidental leakage of API keys, tokens, and passwords
/// in debug logs.
pub fn redact_params_for_log(params: &Value) -> Value {
    redact_value(params)
}

/// Recursively redacts sensitive information from a JSON value.
///
/// It traverses objects and arrays, replacing values of keys that match
/// [`is_sensitive_key`] with `[REDACTED]`.
fn redact_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                if is_sensitive_key(k) {
                    out.insert(k.clone(), Value::String("[REDACTED]".to_string()));
                } else {
                    out.insert(k.clone(), redact_value(v));
                }
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(redact_value).collect()),
        other => other.clone(),
    }
}

/// Returns true if a key name is considered sensitive (e.g., "api_key", "password").
fn is_sensitive_key(key: &str) -> bool {
    matches!(
        key,
        "api_key"
            | "apikey"
            | "token"
            | "access_token"
            | "refresh_token"
            | "authorization"
            | "password"
            | "secret"
            | "client_secret"
    )
}
