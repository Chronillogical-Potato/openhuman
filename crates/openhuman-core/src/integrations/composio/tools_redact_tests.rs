//! Regression coverage for `redact.rs` gaps flagged by review on PR #6689.

use serde_json::json;

use crate::config::{ComposioHostCredential, Config};
use crate::integrations::composio::tools::redact::redact_and_report;
use crate::integrations::composio::tools::redact_composio_outcome;

const SHORT_KEY: &str = "abcdefgh";
const LONG_KEY: &str = "abcdefghi";

fn config_with_overlapping_secrets() -> Config {
    let tmp = tempfile::tempdir().expect("tempdir for config_with_overlapping_secrets");
    let mut config = Config::default();
    config.config_path = tmp.path().join("config.toml");
    config.composio.api_key = Some(LONG_KEY.to_string());
    config
        .composio
        .pin_host_credential(ComposioHostCredential::direct(SHORT_KEY));
    std::mem::forget(tmp);
    config
}

/// Before the fix, `composio_secrets` lexicographically sorted
/// `[SHORT_KEY, LONG_KEY]` (`SHORT_KEY` sorts first since it's a prefix)
/// and redacted in that order, replacing `SHORT_KEY` everywhere first —
/// including inside `LONG_KEY` — and leaving the trailing `i` behind
/// instead of a clean `[REDACTED]`.
#[test]
fn overlapping_secrets_redact_longest_first() {
    let config = config_with_overlapping_secrets();
    let outcome = redact_composio_outcome(
        &config,
        Ok(tinytools::ToolResult::success(format!(
            "token {LONG_KEY} in transit"
        ))),
    )
    .unwrap();
    let text = outcome
        .content
        .iter()
        .filter_map(|c| match c {
            tinytools::ToolContent::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ");
    assert!(!text.contains(LONG_KEY), "{text}");
    assert!(!text.contains(SHORT_KEY), "{text}");
    assert_eq!(text, "token [REDACTED] in transit");
}

/// `redact_value` used to walk only object *values*; a secret that
/// happened to be used as a JSON object key survived untouched.
#[test]
fn secret_used_as_a_json_object_key_is_redacted() {
    let config = config_with_overlapping_secrets();
    let outcome = redact_composio_outcome(
        &config,
        Ok(tinytools::ToolResult::json(
            json!({ format!("token_{LONG_KEY}"): "value" }),
        )),
    )
    .unwrap();
    let serialized = serde_json::to_string(&outcome).unwrap();
    assert!(!serialized.contains(LONG_KEY), "{serialized}");
    assert!(serialized.contains("[REDACTED]"), "{serialized}");
}

#[test]
fn colliding_redacted_object_keys_keep_both_values() {
    let config = config_with_overlapping_secrets();
    let outcome = redact_composio_outcome(
        &config,
        Ok(tinytools::ToolResult::json(json!({
            (SHORT_KEY): "short value",
            (LONG_KEY): "long value",
        }))),
    )
    .unwrap();
    let value = &outcome.content[0];
    let tinytools::ToolContent::Json { data } = value else {
        panic!("expected JSON content");
    };
    let object = data.as_object().expect("JSON object");
    assert_eq!(object.len(), 2);
    assert_eq!(object.get("[REDACTED]").unwrap(), "short value");
    assert_eq!(object.get("[REDACTED] (2)").unwrap(), "long value");
}

/// `redact_and_report` is the single call site that both reports a
/// direct-mode error to observability and supplies the caller's returned
/// error text — one redacted value serves both, so this return value is
/// exactly what reaches `report_composio_op_error`.
#[test]
fn redact_and_report_returns_no_raw_secret() {
    let config = config_with_overlapping_secrets();
    let rendered = format!(
        "[composio-direct] composio_list_connections (direct) failed: invalid credential {LONG_KEY}"
    );
    let redacted = redact_and_report(&config, "test_op", &rendered);
    assert!(!redacted.contains(LONG_KEY), "{redacted}");
    assert!(!redacted.contains(SHORT_KEY), "{redacted}");
    assert!(redacted.contains("[REDACTED]"), "{redacted}");
}

#[test]
fn shared_characterization_corpus_tracks_composio_redaction() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/security-redaction-corpus.json"
    )))
    .unwrap();
    for case in corpus["text"].as_array().unwrap() {
        let input = case["input_parts"]
            .as_array()
            .map(|parts| parts.iter().map(|part| part.as_str().unwrap()).collect::<String>())
            .unwrap_or_else(|| case["input"].as_str().unwrap().to_string());
        let secret = case["composio_secret_parts"]
            .as_array()
            .map(|parts| parts.iter().map(|part| part.as_str().unwrap()).collect::<String>())
            .unwrap_or_else(|| case["composio_secret"].as_str().unwrap().to_string());
        let output = super::redact::redact_text(&input, &[secret.clone()]);
        if input.contains(&secret) {
            assert!(!output.contains(&secret), "case={}: {output}", case["case"]);
        } else {
            assert_eq!(output, input, "case={}", case["case"]);
        }
    }
}
