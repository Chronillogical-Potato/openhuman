use super::*;

/// Every direct web-read tool the budget governs.
const WEB_TOOLS: [&str; 4] = [
    "web_search_tool",
    "web_answer_tool",
    "web_contents_tool",
    "web_fetch",
];

fn request_with_tools(names: &[&str]) -> ModelRequest {
    ModelRequest {
        tools: names
            .iter()
            .map(|name| ToolSchema::new(*name, "tool", json!({})))
            .collect(),
        ..ModelRequest::new(vec![TaMessage::user(
            "Find more information about Jev from TypeSafe".to_string(),
        )])
    }
}

fn research_request() -> ModelRequest {
    request_with_tools(&["web_search_tool"])
}

/// A coding turn: web tools beside the local tools that do the actual work.
fn coding_request() -> ModelRequest {
    let mut names = WEB_TOOLS.to_vec();
    names.extend(["shell", "apply_patch"]);
    request_with_tools(&names)
}

fn tool_names(request: &ModelRequest) -> Vec<&str> {
    request
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect()
}

async fn record_reads(
    mw: &ResearchBudgetMiddleware,
    run: &mut RunContext<crate::agent::tinyagents::host::OpenHumanRunContext>,
    reads: usize,
) {
    for i in 0..reads {
        let name = WEB_TOOLS[i % WEB_TOOLS.len()];
        mw.after_tool(
            run,
            &(),
            &invocation(format!("web-{i}"), name),
            &mut tool_result(name, "Jev is TypeSafe's System One model"),
        )
        .await
        .unwrap();
    }
}

async fn record_failed_read(
    mw: &ResearchBudgetMiddleware,
    run: &mut RunContext<crate::agent::tinyagents::host::OpenHumanRunContext>,
    call_id: &str,
) {
    mw.after_tool(
        run,
        &(),
        &invocation(call_id, "web_fetch"),
        &mut failing_result("web_fetch", "Request failed: connection refused"),
    )
    .await
    .unwrap();
}

/// A run with nothing but web tools still concludes once the budget is spent:
/// there is nothing left to continue the task with.
#[tokio::test]
async fn web_only_research_concludes_after_eight_reads() {
    let mw = ResearchBudgetMiddleware::new();
    let mut run = ctx();
    record_reads(&mw, &mut run, research_budget::DIRECT_WEB_READ_LIMIT - 1).await;
    let mut request = research_request();
    mw.before_model(&mut run, &(), &mut request).await.unwrap();
    assert_eq!(request.tools.len(), 1);

    record_reads(&mw, &mut run, 1).await;
    let mut request = research_request();
    mw.before_model(&mut run, &(), &mut request).await.unwrap();
    assert!(request.tools.is_empty());
    assert_eq!(
        request.tool_choice,
        tinyinference_llm::model::ToolChoice::None
    );
    assert!(request.messages.last().unwrap().text().contains("Answer"));
}

/// With the tools withdrawn, the instruction must say that a tool call will
/// not run: without it DeepSeek V4 wrote its next call as plain-text markup
/// that stood as the answer.
#[tokio::test]
async fn the_concluding_instruction_says_tools_are_gone() {
    let mw = ResearchBudgetMiddleware::new();
    let mut run = ctx();
    record_reads(&mw, &mut run, research_budget::DIRECT_WEB_READ_LIMIT).await;
    let mut request = research_request();
    mw.before_model(&mut run, &(), &mut request).await.unwrap();
    let instruction = request.messages.last().unwrap().text();
    assert_eq!(instruction, research_budget::RESEARCH_CLOSE_INSTRUCTION);
    assert!(instruction.contains("tools are no longer available"));
    assert!(instruction.contains("will not run"));
    assert!(instruction.contains("plain text"));
}

/// #6959: a spent web budget withdraws only the web tools. A coding turn keeps
/// `shell` and `apply_patch` and is told to carry on with them, rather than
/// being forced to answer before any edit is made.
#[tokio::test]
async fn a_spent_web_budget_keeps_the_non_web_tools() {
    let mw = ResearchBudgetMiddleware::new();
    let mut run = ctx();
    record_reads(&mw, &mut run, research_budget::DIRECT_WEB_READ_LIMIT).await;

    let mut request = coding_request();
    mw.before_model(&mut run, &(), &mut request).await.unwrap();
    assert_eq!(tool_names(&request), vec!["shell", "apply_patch"]);
    assert_eq!(
        request.tool_choice,
        tinyinference_llm::model::ToolChoice::Auto
    );
    let instruction = request.messages.last().unwrap().text();
    assert_eq!(
        instruction,
        research_budget::WEB_BUDGET_EXHAUSTED_INSTRUCTION
    );
    assert!(instruction.contains("remaining tools"));

    // Every later request of the turn stays narrowed the same way.
    let mut request = coding_request();
    mw.before_model(&mut run, &(), &mut request).await.unwrap();
    assert_eq!(tool_names(&request), vec!["shell", "apply_patch"]);
}

/// A tool choice pinned to a web tool cannot survive that tool's removal.
#[tokio::test]
async fn a_spent_web_budget_releases_a_choice_pinned_to_a_web_tool() {
    let mw = ResearchBudgetMiddleware::new();
    let mut run = ctx();
    record_reads(&mw, &mut run, research_budget::DIRECT_WEB_READ_LIMIT).await;

    let mut request = coding_request();
    request.tool_choice = tinyinference_llm::model::ToolChoice::Tool("web_fetch".into());
    mw.before_model(&mut run, &(), &mut request).await.unwrap();
    assert_eq!(
        request.tool_choice,
        tinyinference_llm::model::ToolChoice::Auto
    );
}

/// #6959: a failed web call (a blocked network, an offline sandbox) read
/// nothing, so it must not spend the budget.
#[tokio::test]
async fn failed_web_reads_do_not_spend_the_budget() {
    let mw = ResearchBudgetMiddleware::new();
    let mut run = ctx();
    for i in 0..research_budget::DIRECT_WEB_READ_LIMIT * 2 {
        record_failed_read(&mw, &mut run, &format!("fail-{i}")).await;
    }
    let mut request = coding_request();
    mw.before_model(&mut run, &(), &mut request).await.unwrap();
    assert_eq!(request.tools.len(), WEB_TOOLS.len() + 2);
    assert_ne!(
        request.messages.last().unwrap().text(),
        research_budget::WEB_BUDGET_EXHAUSTED_INSTRUCTION
    );
}

/// Two web failures in a row get one note that web access looks blocked, on
/// the next request only.
#[tokio::test]
async fn consecutive_failed_web_reads_note_that_web_access_looks_blocked_once() {
    let mw = ResearchBudgetMiddleware::new();
    let mut run = ctx();

    record_failed_read(&mw, &mut run, "fail-0").await;
    let mut request = coding_request();
    mw.before_model(&mut run, &(), &mut request).await.unwrap();
    assert_eq!(request.messages.len(), 1, "one failure is not a pattern");

    record_failed_read(&mw, &mut run, "fail-1").await;
    let mut request = coding_request();
    mw.before_model(&mut run, &(), &mut request).await.unwrap();
    assert_eq!(
        request.messages.last().unwrap().text(),
        research_budget::WEB_BLOCKED_NOTE
    );
    assert_eq!(request.tools.len(), WEB_TOOLS.len() + 2);

    record_failed_read(&mw, &mut run, "fail-2").await;
    record_failed_read(&mw, &mut run, "fail-3").await;
    let mut request = coding_request();
    mw.before_model(&mut run, &(), &mut request).await.unwrap();
    assert_eq!(request.messages.len(), 1, "the note is sent once per run");
}

/// A successful read in between breaks the run of failures.
#[tokio::test]
async fn a_successful_read_resets_the_failure_streak() {
    let mw = ResearchBudgetMiddleware::new();
    let mut run = ctx();
    record_failed_read(&mw, &mut run, "fail-0").await;
    record_reads(&mw, &mut run, 1).await;
    record_failed_read(&mw, &mut run, "fail-1").await;

    let mut request = coding_request();
    mw.before_model(&mut run, &(), &mut request).await.unwrap();
    assert_eq!(request.messages.len(), 1);
}

#[tokio::test]
async fn unrelated_tools_do_not_spend_the_web_research_budget() {
    let mw = ResearchBudgetMiddleware::new();
    let mut run = ctx();
    for i in 0..10 {
        mw.after_tool(
            &mut run,
            &(),
            &invocation(format!("file-{i}"), "file_read"),
            &mut tool_result("file_read", "content"),
        )
        .await
        .unwrap();
    }
    let mut request = research_request();
    mw.before_model(&mut run, &(), &mut request).await.unwrap();
    assert_eq!(request.tools.len(), 1);
}
