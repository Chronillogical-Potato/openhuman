use super::*;

#[test]
fn desktop_arguments_reject_missing_or_blank_required_values() {
    assert_eq!(
        required(&json!({"app":"  TextEdit  "}), "app").unwrap(),
        "TextEdit"
    );
    for args in [json!({}), json!({"app":"  "}), json!({"app":42})] {
        assert!(required(&args, "app")
            .unwrap_err()
            .to_string()
            .contains("missing required parameter: app"));
    }
}

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
fn raw_ref_action_requires_a_snapshot_ref_and_declares_a_write_effect() {
    let tool = DesktopTool::new(Arc::new(Config::default()), DesktopToolKind::Act);
    let schema = tool.parameters_schema();
    assert_eq!(tool.name(), "desktop_act");
    assert_eq!(tool.permission_level(), PermissionLevel::Write);
    assert!(tool.external_effect());
    assert_eq!(schema["required"], json!(["operation", "ref_id"]));
    assert_eq!(
        schema["properties"]["operation"]["enum"],
        json!(["click", "focus", "type", "check", "uncheck", "expand", "collapse"])
    );
    assert!(schema["properties"].get("args").is_none());
    assert!(schema["properties"].get("env").is_none());
}

#[test]
fn snapshot_can_bind_the_window_listed_by_desktop() {
    let tool = DesktopTool::new(Arc::new(Config::default()), DesktopToolKind::Snapshot);
    assert_eq!(tool.exposure(), ToolExposure::Deferred);
    assert!(tool.parameters_schema()["properties"]
        .get("window_id")
        .is_some());
    let request = snapshot_request(&json!({"app":"TextEdit","window_id":"w-515619"})).unwrap();
    assert_eq!(request.app.as_deref(), Some("TextEdit"));
    assert_eq!(request.window_id.as_deref(), Some("w-515619"));
    assert!(snapshot_request(&json!({"window_id":"  "})).is_err());
}

#[test]
fn goal_and_continuation_have_distinct_required_inputs() {
    let config = Arc::new(Config::default());
    let goal = DesktopTool::new(config.clone(), DesktopToolKind::Goal);
    let continuation = DesktopTool::new(config, DesktopToolKind::ContinueGoal);
    assert_eq!(
        goal.parameters_schema()["required"],
        json!([
            "app",
            "goal",
            "allowed_operations",
            "allowed_targets",
            "success"
        ])
    );
    assert!(goal.parameters_schema()["properties"]
        .get("confirmation_id")
        .is_none());
    assert!(
        !goal.parameters_schema()["properties"]["success"]["items"]["properties"]["kind"]["enum"]
            .as_array()
            .unwrap()
            .contains(&json!("name_absent"))
    );
    assert_eq!(
        continuation.parameters_schema()["required"],
        json!(["confirmation_id"])
    );
    assert!(continuation.parameters_schema()["properties"]
        .get("approve")
        .is_none());
    assert!(super::super::confirmation::take_approved("unknown", Some("thread-a")).is_err());
}

#[test]
fn goal_request_carries_scoped_task_and_disables_confirmations() {
    let args = json!({
        "app":"TextEdit", "window":"Untitled", "window_id":"w-515619",
        "goal":"Enter a disposable marker",
        "allowed_operations":["TYPE_TEXT"], "allowed_targets":["Text Entry Area"],
        "text_slots":{"Text Entry Area":"desktop-test-42"},
        "success":[{"kind":"value_equals","name":"Text Entry Area","value":"desktop-test-42"}],
        "max_steps":3, "max_model_calls":5, "max_elapsed_ms":20_000
    });
    let request = goal_request(&args, false).unwrap();
    assert_eq!(request["require_confirmations"], false);
    assert_eq!(request["window"], "Untitled");
    assert_eq!(request["window_id"], "w-515619");
    let goal = DesktopTool::new(Arc::new(Config::default()), DesktopToolKind::Goal);
    assert!(goal.parameters_schema()["properties"]
        .get("window_id")
        .is_some());
    assert!(
        goal.parameters_schema()["properties"]["success"]["items"]["properties"]["kind"]["enum"]
            .as_array()
            .unwrap()
            .contains(&json!("name_contains"))
    );
    assert_eq!(request["allowed_operations"], json!(["TYPE_TEXT"]));
    assert_eq!(request["allowed_targets"], json!(["Text Entry Area"]));
    assert_eq!(request["text_slots"]["Text Entry Area"], "desktop-test-42");
    assert_eq!(request["success"], args["success"]);
    assert_eq!(request["max_elapsed_ms"], 20_000);
    assert!(request["continuation"].is_null());
    assert_eq!(
        goal_request(&args, true).unwrap()["require_confirmations"],
        true
    );
    assert!(goal_request(&json!({"app":"TextEdit","goal":"type"}), false).is_err());
    let without_targets = json!({"app":"TextEdit","goal":"type", "allowed_operations":["TYPE_TEXT"],
        "success":[{"kind":"name_present","name":"Text Entry Area"}]});
    assert!(goal_request(&without_targets, false)
        .unwrap_err()
        .to_string()
        .contains("needs_inspection"));
    let mut blank_target = without_targets.clone();
    blank_target["allowed_targets"] = json!(["  "]);
    assert!(goal_request(&blank_target, false).is_err());
    let mut blank_window = args.clone();
    blank_window["window_id"] = json!("  ");
    assert!(goal_request(&blank_window, false).is_err());
    let malformed = json!({"app":"TextEdit","goal":"type", "allowed_operations":["TYPE_TEXT"],
        "allowed_targets":["Text Entry Area"],
        "success":[{"kind":"value_contains","name":"Text Entry Area"}]});
    assert!(goal_request(&malformed, false).is_err());
    for predicate in [
        json!({"kind":"name_absent","name":"Play"}),
        json!({"kind":"name_present","name":"  "}),
        json!({"kind":"value_contains","name":"Text Entry Area","value":""}),
        json!({"kind":"value_equals","name":"","value":"marker"}),
        json!({"kind":"state_contains","name":"Play","state":" "}),
    ] {
        let mut request = args.clone();
        request["success"] = json!([predicate]);
        assert!(goal_request(&request, false).is_err());
    }
    let mut cleared = args.clone();
    cleared["success"] = json!([{"kind":"value_equals","name":"Text Entry Area","value":""}]);
    assert!(goal_request(&cleared, false).is_ok());

    let outbound = json!({
        "app":"WhatsApp", "goal":"Send exactly the prepared text to Alex Rivera",
        "allowed_operations":["CLICK","TYPE_TEXT"],
        "allowed_targets":["Compose message","Send"],
        "text_slots":{"Compose message":"Hello from OpenHuman!  "},
        "success":[
            {"kind":"name_present","name":"Messages in chat with Alex Rivera"},
            {"kind":"name_contains","fragment":"Your message, Hello from OpenHuman!  ","within":"Messages in chat with Alex Rivera"}
        ]
    });
    let prepared = goal_request(&outbound, false).unwrap();
    assert_eq!(
        prepared["text_slots"]["Compose message"],
        "Hello from OpenHuman!  "
    );
    assert_eq!(
        prepared["success"][1]["fragment"],
        "Your message, Hello from OpenHuman!  "
    );
    let mut wrong_slot = outbound.clone();
    wrong_slot["text_slots"] = json!({"Other field":"Hello from OpenHuman!  "});
    assert!(goal_request(&wrong_slot, false).is_err());
    let mut missing_scope = outbound;
    missing_scope["success"][1]["within"] = json!(" ");
    assert!(goal_request(&missing_scope, false).is_err());
}

#[test]
fn goal_timeout_outlives_the_module_loop_budget() {
    let goal = DesktopTool::new(Arc::new(Config::default()), DesktopToolKind::Goal);
    assert_eq!(
        goal.timeout_policy(&json!({})),
        ToolTimeout::Millis(150_000)
    );
    assert_eq!(
        goal.timeout_policy(&json!({"max_elapsed_ms":300_000})),
        ToolTimeout::Millis(330_000)
    );
    assert_eq!(
        goal.timeout_policy(&json!({"max_elapsed_ms":999_999})),
        ToolTimeout::Millis(330_000)
    );
}
