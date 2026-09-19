use super::*;
use tinytools::{ToolCallOptions, ToolRunContext};

struct ThreadContext(&'static str);

impl ToolRunContext for ThreadContext {
    fn thread_id(&self) -> Option<&str> {
        Some(self.0)
    }
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
    assert!(res.text().contains("land the PR"));

    let get = GoalGetTool::new(dir.clone());
    let res = get
        .execute_with_context(json!({}), ToolCallOptions::default(), Some(&context))
        .await
        .unwrap();
    assert!(res.text().contains("status: active"));

    let done = GoalCompleteTool::new(dir.clone());
    let res = done
        .execute_with_context(json!({}), ToolCallOptions::default(), Some(&context))
        .await
        .unwrap();
    assert!(res.text().contains("status: complete"));
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
    assert!(res.text().contains("no goal set"));
}
