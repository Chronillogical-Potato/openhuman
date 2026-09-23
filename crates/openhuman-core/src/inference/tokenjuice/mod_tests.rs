use super::*;

#[test]
fn recovery_tool_aliases_remain_stable() {
    assert!(is_recovery_tool(RETRIEVE_TOOL_NAME));
    assert!(is_recovery_tool(LEGACY_RETRIEVE_TOOL_NAME));
    assert!(!is_recovery_tool("shell"));
}

#[tokio::test]
async fn disabled_compaction_is_an_exact_pass_through_without_loading_the_module() {
    let content = "exact tool output".to_string();
    let output = compact_output_with_policy(
        content.clone(),
        "shell",
        false,
        AgentTokenjuiceCompression::Full,
    )
    .await;
    assert_eq!(output, content);
}

#[tokio::test]
async fn off_profile_is_an_exact_pass_through_without_loading_the_module() {
    let content = "exact tool output".to_string();
    let output = compact_output_with_policy(
        content.clone(),
        "shell",
        true,
        AgentTokenjuiceCompression::Off,
    )
    .await;
    assert_eq!(output, content);
}

/// The whole summary path over the real bus: `CompactWith` into the module,
/// `MlHost.Generate` back out to a registered call, the summary back in.
/// Runs where CI builds the module (`TINYJUICE_TEST_MODULE`); skipped
/// otherwise, since the pinned release may predate `CompactWith`.
#[tokio::test]
async fn the_module_calls_back_for_a_summary_written_for_the_focus() {
    if std::env::var_os("TINYJUICE_TEST_MODULE").is_none() {
        return;
    }
    let seen = std::sync::Arc::new(std::sync::Mutex::new(None::<types::GenerateRequest>));
    let sink = seen.clone();
    let ticket = generate::register(Box::new(move |request: types::GenerateRequest| {
        *sink.lock().unwrap() = Some(request);
        Box::pin(async { Ok("the rate limit is 60 requests a minute".to_string()) })
    }));
    // Over the default 4000-token threshold the test config installs.
    let content = "Rate limiting. Requests are limited per key. ".repeat(500);

    let output = compact_tool_output(ToolOutputCompaction {
        content: content.clone(),
        tool_name: "web_fetch",
        enabled: false,
        profile: AgentTokenjuiceCompression::Full,
        runtime_config: None,
        arguments: None,
        focus: Some("the rate limits".into()),
        context_token: Some(ticket.token().to_string()),
        scope: Some("module-summary-test".into()),
    })
    .await;

    assert_eq!(output.summarized_from_bytes, Some(content.len()));
    assert!(output
        .text
        .starts_with("the rate limit is 60 requests a minute"));
    assert!(
        output.text.contains(RETRIEVE_TOOL_NAME),
        "the original stays retrievable: {}",
        output.text
    );
    let request = seen
        .lock()
        .unwrap()
        .clone()
        .expect("the module called back");
    assert!(request.prompt.contains("Caller focus: the rate limits"));
    assert!(request.system.contains("caller focus"));
}
