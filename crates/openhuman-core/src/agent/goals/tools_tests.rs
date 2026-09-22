use super::*;
use tinytools::{ToolCallOptions, ToolRunContext};

struct ThreadContext(&'static str);

impl ToolRunContext for ThreadContext {
    fn thread_id(&self) -> Option<&str> {
        Some(self.0)
    }
}

/// Every goal tool answers with `{ goal, text }`: the structured goal the UI
/// reads and the rendered block the transcript shows.
fn payload(res: &tinytools::ToolResult) -> serde_json::Value {
    serde_json::from_str(&res.text()).unwrap_or_else(|e| panic!("goal payload is JSON: {e}: {}", res.text()))
}

#[tokio::test]
async fn set_get_complete_via_tools_in_thread_scope() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().to_path_buf();
    let context = ThreadContext("thread-tools");
    let set = GoalSetTool::new(dir.clone());
    let res = set
        .execute_with_context(
            json!({ "objective": "land the PR", "token_budget": 5000 }),
            ToolCallOptions::default(),
            Some(&context),
        )
        .await
        .unwrap();
    assert!(!res.is_error, "{}", res.text());
    let set_payload = payload(&res);
    assert_eq!(set_payload["goal"]["objective"], "land the PR");
    assert_eq!(set_payload["goal"]["status"], "active");
    assert_eq!(set_payload["goal"]["tokenBudget"], 5000);
    assert_eq!(set_payload["goal"]["tokensUsed"], 0);
    let text = set_payload["text"].as_str().unwrap();
    assert!(text.starts_with("Goal set."), "{text}");
    assert!(text.contains("objective: land the PR"), "{text}");

    let get = GoalGetTool::new(dir.clone());
    let res = get
        .execute_with_context(json!({}), ToolCallOptions::default(), Some(&context))
        .await
        .unwrap();
    let get_payload = payload(&res);
    assert_eq!(get_payload["goal"]["status"], "active");
    assert_eq!(get_payload["goal"]["goalId"], set_payload["goal"]["goalId"]);
    assert!(get_payload["text"].as_str().unwrap().contains("status: active"));

    let done = GoalCompleteTool::new(dir.clone());
    let res = done
        .execute_with_context(json!({}), ToolCallOptions::default(), Some(&context))
        .await
        .unwrap();
    let done_payload = payload(&res);
    assert_eq!(done_payload["goal"]["status"], "complete");
    assert!(done_payload["text"].as_str().unwrap().starts_with("Goal marked complete."));
}

#[tokio::test]
async fn tools_error_without_thread_scope() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().to_path_buf();
    let set = GoalSetTool::new(dir.clone());
    let res = set.execute(json!({ "objective": "x" })).await.unwrap();
    assert!(res.is_error);
    assert!(res.text().contains("active chat thread"));
}

#[tokio::test]
async fn get_reports_absent_goal() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().to_path_buf();
    let context = ThreadContext("empty-thread");
    let get = GoalGetTool::new(dir.clone());
    let res = get
        .execute_with_context(json!({}), ToolCallOptions::default(), Some(&context))
        .await
        .unwrap();
    let absent = payload(&res);
    assert!(absent["goal"].is_null(), "{absent}");
    assert_eq!(absent["text"], "no goal set for this thread");
}
