use super::*;

/// The managed `openhuman.{billing,usage}` envelope on `raw` must re-project
/// into the host `BilledUsage` the cost bridge reads — charged USD, cached
/// tokens, and context window — exactly as the legacy legacy model-adapter path did.
#[test]
fn project_managed_usage_recovers_charged_and_cached() {
    use crate::agent::tinyagents::model::usage_info_from_response;
    use tinyinference_llm::message::AssistantMessage;
    use tinyinference_llm::usage::Usage;

    let raw = serde_json::json!({
        "openhuman": {
            "usage": { "cached_input_tokens": 128, "context_window": 200000 },
            "billing": { "charged_amount_usd": 0.0042 }
        }
    });
    let response = ModelResponse {
        message: AssistantMessage {
            id: None,
            content: vec![],
            tool_calls: vec![],
            usage: None,
            origin: None,
        },
        usage: Some(Usage {
            input_tokens: 1000,
            output_tokens: 50,
            ..Usage::default()
        }),
        finish_reason: None,
        raw: Some(raw),
        resolved_model: None,
        continue_turn: None,
        served_from_cache: false,
        correlation: None,
        resolved_route: None,
    };

    let projected = project_managed_usage(response);
    let usage = usage_info_from_response(&projected).expect("usage recovered");
    assert!(
        (usage.charged_amount_usd - 0.0042).abs() < 1e-9,
        "charged={}",
        usage.charged_amount_usd
    );
    assert_eq!(usage.cached_input_tokens(), 128, "cached tokens backfilled");
    assert_eq!(usage.context_window(), 200_000);
    assert_eq!(usage.input_tokens, 1000);
    assert_eq!(usage.output_tokens, 50);
}

/// A response with no `openhuman` envelope stays untouched — no meta key, no
/// charged USD — so non-managed/billing-free responses aren't fabricated.
#[test]
fn project_managed_usage_is_noop_without_envelope() {
    use crate::agent::tinyagents::model::usage_info_from_response;
    use tinyinference_llm::message::AssistantMessage;
    use tinyinference_llm::usage::Usage;

    let response = ModelResponse {
        message: AssistantMessage {
            id: None,
            content: vec![],
            tool_calls: vec![],
            usage: None,
            origin: None,
        },
        usage: Some(Usage {
            input_tokens: 10,
            output_tokens: 5,
            cache_read_tokens: 3,
            ..Usage::default()
        }),
        finish_reason: None,
        raw: Some(serde_json::json!({ "id": "resp_1" })),
        resolved_model: None,
        continue_turn: None,
        served_from_cache: false,
        correlation: None,
        resolved_route: None,
    };

    let projected = project_managed_usage(response);
    // raw keeps only the wire fields — no meta key injected.
    assert!(projected
        .raw
        .as_ref()
        .unwrap()
        .get("openhuman_usage_meta")
        .is_none());
    let usage = usage_info_from_response(&projected).expect("usage present");
    assert_eq!(usage.charged_amount_usd, 0.0);
    assert_eq!(usage.cache_read_tokens, 3, "crate cached preserved");
}
