use super::*;

fn research_request() -> ModelRequest {
    ModelRequest {
        tools: vec![ToolSchema::new("web_search_tool", "search", json!({}))],
        ..ModelRequest::new(vec![TaMessage::user(
            "Find more information about Jev from TypeSafe".to_string(),
        )])
    }
}

#[tokio::test]
async fn web_research_concludes_after_eight_reads() {
    let mw = ResearchBudgetMiddleware::new();
    let mut run = ctx();
    for i in 0..7 {
        let name = if i % 2 == 0 {
            "web_search_tool"
        } else {
            "web_fetch"
        };
        mw.after_tool(
            &mut run,
            &(),
            &invocation(format!("web-{i}"), name),
            &mut tool_result(name, "Jev is TypeSafe's System One model"),
        )
        .await
        .unwrap();
    }
    let mut request = research_request();
    mw.before_model(&mut run, &(), &mut request).await.unwrap();
    assert_eq!(request.tools.len(), 1);

    mw.after_tool(
        &mut run,
        &(),
        &invocation("web-7", "web_fetch"),
        &mut tool_result("web_fetch", "TypeSafe's announcement"),
    )
    .await
    .unwrap();
    let mut request = research_request();
    mw.before_model(&mut run, &(), &mut request).await.unwrap();
    assert!(request.tools.is_empty());
    assert_eq!(
        request.tool_choice,
        tinyinference_llm::model::ToolChoice::None
    );
    assert!(request.messages.last().unwrap().text().contains("Answer"));
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
