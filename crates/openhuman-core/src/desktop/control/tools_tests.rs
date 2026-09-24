use super::*;

#[tokio::test]
async fn deferred_tool_is_discoverable_but_fails_closed_when_disabled() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.workspace_dir = dir.path().to_path_buf();
    let tool = DesktopTool::new(Arc::new(config), DesktopToolKind::Apps);
    assert_eq!(tool.exposure(), ToolExposure::Deferred);
    let result = tool.execute(json!({})).await.unwrap();
    assert!(result.output().contains("disabled in Connections"));
}

#[test]
fn launch_tool_cannot_accept_process_arguments_or_environment() {
    let tool = DesktopTool::new(Arc::new(Config::default()), DesktopToolKind::Launch);
    let schema = tool.parameters_schema();
    assert_eq!(tool.exposure(), ToolExposure::Deferred);
    assert_eq!(tool.permission_level(), PermissionLevel::Write);
    assert!(tool.external_effect());
    assert_eq!(schema["required"], json!(["app"]));
    assert!(schema["properties"].get("args").is_none());
    assert!(schema["properties"].get("env").is_none());
    assert!(schema["properties"].get("cdp_port").is_none());
}

#[test]
fn goal_and_continuation_have_distinct_required_inputs() {
    let config = Arc::new(Config::default());
    let goal = DesktopTool::new(config.clone(), DesktopToolKind::Goal);
    let continuation = DesktopTool::new(config, DesktopToolKind::ContinueGoal);
    assert_eq!(goal.parameters_schema()["required"], json!(["app", "goal"]));
    assert!(goal.parameters_schema()["properties"]
        .get("confirmation_id")
        .is_none());
    assert_eq!(
        continuation.parameters_schema()["required"],
        json!(["confirmation_id"])
    );
    assert!(continuation.parameters_schema()["properties"]
        .get("approve")
        .is_none());
    assert!(super::super::confirmation::take_approved("unknown", Some("thread-a")).is_err());
}

#[tokio::test]
async fn approvals_off_continues_one_use_handle_and_reports_module_result() {
    assert!(!Config::default().desktop.approvals_enabled);
    let called = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = Arc::clone(&called);
    let initial = DesktopResponse::ok(
        "run-goal",
        json!({
            "stop":"confirmation_required", "confirmation_id":"once"
        }),
    );
    let result = advance_goal_confirmations(initial, false, move |id, approve| {
        let observed = Arc::clone(&observed);
        async move {
            assert_eq!(id, "once");
            assert!(approve);
            observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(DesktopResponse::ok(
                "run-goal",
                json!({
                    "stop":"stale_target", "turns":[], "metrics":{"calls":1}
                }),
            ))
        }
    })
    .await
    .unwrap();
    assert_eq!(called.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(result.data.unwrap()["stop"], "stale_target");
}

#[tokio::test]
async fn approvals_on_returns_pending_without_self_approval() {
    let initial = DesktopResponse::ok(
        "run-goal",
        json!({
            "stop":"confirmation_required", "confirmation_id":"once"
        }),
    );
    let result = advance_goal_confirmations(initial, true, |_id, _approve| async {
        panic!("manual approval mode must never auto-continue")
    })
    .await
    .unwrap();
    assert_eq!(result.data.unwrap()["confirmation_id"], "once");
}

#[tokio::test]
async fn continuation_ceiling_returns_cancelled_result_with_all_executed_turns() {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let seen = Arc::clone(&calls);
    let initial = DesktopResponse::ok(
        "run-goal",
        json!({
            "stop":"confirmation_required", "confirmation_id":"id-0", "turns":[]
        }),
    );
    let result = advance_goal_confirmations(initial, false, move |id, approve| {
        let seen = Arc::clone(&seen);
        async move {
            let index = seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            assert_eq!(id, format!("id-{index}"));
            if approve {
                Ok(DesktopResponse::ok(
                    "run-goal",
                    json!({
                        "stop":"confirmation_required",
                        "confirmation_id":format!("id-{}", index + 1),
                        "turns":vec![Value::Null; index + 1],
                        "metrics":{"calls":index + 1}
                    }),
                ))
            } else {
                assert_eq!(index, 8);
                Ok(DesktopResponse::ok(
                    "run-goal",
                    json!({
                        "stop":"cancelled", "turns":vec![Value::Null; 8],
                        "metrics":{"calls":8}
                    }),
                ))
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 9);
    let data = result.data.unwrap();
    assert_eq!(data["stop"], "cancelled");
    assert_eq!(data["turns"].as_array().unwrap().len(), 8);
    assert_eq!(data["metrics"]["calls"], 8);
}
