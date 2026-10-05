use super::*;
use crate::config::{SearchProviderSettings, SearchRoute};

fn signed_out_byok_config() -> Config {
    let mut config = Config::default();
    config.search.providers = [
        ("brave".to_string(), SearchProviderSettings::direct()),
        ("tavily".to_string(), SearchProviderSettings::direct()),
    ]
    .into_iter()
    .collect();
    config.search.brave.api_key = Some("b".into());
    config.search.tavily.api_key = Some("t".into());
    config
}

#[test]
fn classified_errors_become_actionable_messages() {
    let balance = user_facing_error("tinysearch.insufficient_balance: 402 from backend");
    assert!(balance.contains("balance is too low"));
    assert!(user_facing_error("tinysearch.rate_limited: slow down").contains("rate limited"));
    assert_eq!(
        user_facing_error("tinysearch.provider_unavailable: all down"),
        SEARCH_EXHAUSTED_MESSAGE
    );
    assert_eq!(
        user_facing_error(
            "search ExecuteTool failed: tinysearch.invalid_arguments: urls must not be empty"
        ),
        "The search request was rejected: urls must not be empty"
    );
    assert_eq!(user_facing_error("boom"), "Web search failed: boom");
    assert_eq!(
        error_code("x tinysearch.rate_limited: y"),
        Some("rate_limited")
    );
    assert_eq!(error_code("plain"), None);
}

#[test]
fn role_tools_are_built_for_usable_byok_providers() {
    let config = signed_out_byok_config();
    let tools = build_search_tools(&config);
    let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
    assert!(names.contains(&"web_search_tool"), "{names:?}");
    assert!(names.contains(&"web_contents_tool"), "{names:?}");
    // Nothing usable can answer without a session or a Gemini key.
    assert!(!names.contains(&"web_answer_tool"), "{names:?}");
    let search = tools
        .iter()
        .find(|t| t.name() == "web_search_tool")
        .unwrap();
    assert_eq!(search.category(), ToolCategory::Workflow);
    assert!(search.supports_markdown());
    assert!(search.parameters_schema()["properties"]["query"].is_object());
}

#[test]
fn no_tools_when_search_is_off_or_nothing_is_usable() {
    let mut config = signed_out_byok_config();
    config.search.enabled = Some(false);
    assert!(build_search_tools(&config).is_empty());

    let mut managed_only = Config::default();
    managed_only.search.providers.insert(
        "exa".into(),
        SearchProviderSettings {
            enabled: true,
            route: SearchRoute::Managed,
        },
    );
    // No backend credential in a default test config.
    assert!(build_search_tools(&managed_only).is_empty());
}

#[test]
fn recorded_tools_keep_their_declaration() {
    let spec = ToolSpec {
        name: "web_answer_tool".into(),
        description: "Grounded answers".into(),
        parameters: serde_json::json!({"type": "object"}),
    };
    let tool = TinySearchTool::recorded(spec.clone());
    assert_eq!(tool.name(), "web_answer_tool");
    assert_eq!(tool.spec(), &spec);
    assert_eq!(tool.exposure(), ToolExposure::Direct);
}

#[test]
fn local_only_blocks_external_search_tool_dispatch() {
    let _privacy =
        crate::security::live_policy::test_privacy_scope(crate::config::PrivacyMode::LocalOnly);

    let message = local_only_search_block("web_search_tool")
        .expect("search requests must be blocked in LocalOnly mode");
    assert!(message.contains(crate::security::POLICY_BLOCKED_MARKER));
}

#[test]
fn standard_privacy_mode_allows_search_tool_dispatch() {
    let _privacy =
        crate::security::live_policy::test_privacy_scope(crate::config::PrivacyMode::Standard);

    assert!(local_only_search_block("web_search_tool").is_none());
}

// ---------------------------------------------------------------------------
// #6991: a credential alone makes the managed route look reachable, so a
// deployment that is offline, firewalled, out of balance or holding a dead key
// offers these tools on every turn and fails every call. The old message ended
// "unavailable right now", which reads as "retry later": in the DeepSWE-10 run
// the agent called search three times on one task, then guessed, and the guess
// was what the hidden test rejected.
// ---------------------------------------------------------------------------

fn role_specs() -> Vec<ToolSpec> {
    ["web_search_tool", "web_answer_tool", "web_contents_tool"]
        .into_iter()
        .map(|name| ToolSpec {
            name: name.to_string(),
            description: "search".into(),
            parameters: serde_json::json!({"type": "object"}),
        })
        .collect()
}

#[test]
fn the_unavailable_verdict_tells_the_model_to_stop_rather_than_wait() {
    let message = user_facing_error("tinysearch.provider_unavailable: all down");

    assert!(
        message.contains("Do not call it again"),
        "the model must be told to stop, got: {message}"
    );
    assert!(
        !message.contains("right now"),
        "nothing in the session will change the answer, so it must not read as a retry hint: {message}"
    );
}

#[test]
fn only_the_exhaustion_verdict_is_treated_as_final() {
    // A rate limit clears on its own; a low balance and a rejected argument
    // each have a message naming what to change. None of them may latch.
    assert!(exhausts_providers(
        "tinysearch.provider_unavailable: all down"
    ));
    assert!(!exhausts_providers("tinysearch.rate_limited: slow down"));
    assert!(!exhausts_providers("tinysearch.insufficient_balance: 402"));
    assert!(!exhausts_providers(
        "tinysearch.invalid_arguments: urls must not be empty"
    ));
    assert!(!exhausts_providers("boom"));
}

#[test]
fn a_session_starts_unlatched() {
    for tool in TinySearchTool::recorded_batch(role_specs()) {
        assert!(!tool.is_exhausted(), "{} started latched", tool.name());
    }
}

#[test]
fn one_role_finding_nothing_settles_it_for_the_others() {
    // The three roles are declarations over the same providers, so dropping
    // only the role that failed would still leave two tools that cannot work.
    let tools = TinySearchTool::recorded_batch(role_specs());

    tools[0].mark_exhausted();

    for tool in &tools {
        assert!(
            tool.is_exhausted(),
            "{} did not share the latch",
            tool.name()
        );
    }
}

#[tokio::test]
async fn a_latched_tool_refuses_before_reaching_the_module() {
    // The refusal is returned without loading config or calling the module, so
    // a dead search costs the session one round trip rather than one per call.
    let tools = TinySearchTool::recorded_batch(role_specs());
    tools[1].mark_exhausted();

    let result = tools[0]
        .execute(serde_json::json!({"query": "anything"}))
        .await
        .expect("a refusal is a reported failure, not a tool error");

    assert!(result.is_error);
    assert_eq!(
        result.error_kind,
        Some(tinytools::ToolErrorKind::Failed),
        "the failure is permanent, so it must not be tagged retryable"
    );
    assert!(
        result.text().contains("Do not call it again"),
        "{:?}",
        result.text()
    );
}
