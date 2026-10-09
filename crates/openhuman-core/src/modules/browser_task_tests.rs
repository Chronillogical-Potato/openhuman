use super::*;
use serde_json::json;
use tinycomputer_bus::agent::AgentError;

fn task() -> BrowserTask {
    BrowserTask {
        goal: "Find the opening hours".into(),
        facts: BTreeMap::from([("name".into(), "Asha".into())]),
        origins: vec!["https://.example.com".into()],
        max_actions: 12,
        flow: None,
        site: None,
    }
}

#[test]
fn start_request_confines_the_task_to_the_browser_under_host_policy() {
    let mut config = Config::default();
    config.browser.headless = false;
    config.browser.task_timeout_secs = 90;
    config.computer.max_rescues = Some(2);
    let request = start_request(&config, &task());
    assert_eq!(request.task.as_deref(), Some("Find the opening hours"));
    assert_eq!(request.facts["name"], "Asha");
    assert_eq!(request.constraints.surfaces, vec![SurfaceKind::Browser]);
    assert_eq!(request.constraints.origins, vec!["https://.example.com"]);
    assert_eq!(request.constraints.payment, PaymentMode::StopAtPayment);
    assert!(!request.constraints.allow_destructive);
    assert!(request.constraints.headed);
    assert_eq!(request.budget.max_actions, Some(12));
    assert_eq!(request.budget.max_elapsed_ms, Some(90_000));
    assert_eq!(request.budget.max_rescues, Some(2));
}

#[test]
fn a_traced_host_asks_the_module_to_record_every_decision() {
    let mut config = Config::default();
    config.computer.trace = true;
    assert!(start_request(&config, &task()).trace);
}

#[tokio::test]
async fn start_refuses_a_task_without_origins() {
    let mut empty = task();
    empty.origins.clear();
    let error = start(&Config::default(), &empty).await.unwrap_err();
    assert!(error.contains("allowed origin"), "{error}");
}

#[test]
fn only_the_module_refusing_a_flow_reads_as_a_refused_flow() {
    let refused: AgentResponse<u32> = AgentResponse {
        ok: false,
        data: None,
        error: Some(AgentError::new(
            "INVALID_FLOW",
            "step 2 is not a step",
            "fix the flow",
            true,
        )),
    };
    assert!(refused_flow(
        &unwrap_response("StartTask", refused).unwrap_err()
    ));
    let other: AgentResponse<u32> = AgentResponse {
        ok: false,
        data: None,
        error: Some(AgentError::new(
            "PLANNER_UNAVAILABLE",
            "no planner",
            "configure one",
            false,
        )),
    };
    assert!(!refused_flow(
        &unwrap_response("StartTask", other).unwrap_err()
    ));
    // Only the code counts, not a message that quotes it.
    let quoted: AgentResponse<u32> = AgentResponse {
        ok: false,
        data: None,
        error: Some(AgentError::new(
            "PLANNER_UNAVAILABLE",
            "[INVALID_FLOW] was the last answer",
            "configure one",
            false,
        )),
    };
    assert!(!refused_flow(
        &unwrap_response("StartTask", quoted).unwrap_err()
    ));
}

#[test]
fn unwrap_response_returns_data_or_the_structured_error() {
    let ok = AgentResponse {
        ok: true,
        data: Some(7),
        error: None,
    };
    assert_eq!(unwrap_response("StartTask", ok), Ok(7));
    let failed: AgentResponse<u32> = AgentResponse {
        ok: false,
        data: None,
        error: Some(AgentError::new(
            "PLANNER_UNAVAILABLE",
            "no planner",
            "configure one",
            false,
        )),
    };
    let error = unwrap_response("StartTask", failed).unwrap_err();
    assert!(error.contains("PLANNER_UNAVAILABLE"), "{error}");
    let empty: AgentResponse<u32> = AgentResponse {
        ok: true,
        data: None,
        error: None,
    };
    assert!(unwrap_response("AwaitTask", empty)
        .unwrap_err()
        .contains("no result"));
}

#[test]
fn a_saved_flow_is_run_instead_of_planned() {
    let flow: tinycomputer_bus::Flow = serde_json::from_value(serde_json::json!({
        "app": "browser",
        "steps": [{"browse": "https://www.example.com/"}, {"stop_before": "paying"}]
    }))
    .unwrap();
    let mut saved = task();
    saved.flow = Some(flow.clone());
    let request = start_request(&Config::default(), &saved);
    assert_eq!(request.flow, Some(flow));
    assert_eq!(request.task.as_deref(), Some("Find the opening hours"));
}

#[tokio::test]
async fn a_task_that_already_stopped_settles_without_calling_the_module() {
    // Not running, so no `AwaitTask`; finished and untraced, so no report is
    // fetched either.
    let mut config = Config::default();
    config.computer.trace = false;
    let view: TaskView = serde_json::from_value(json!({
        "id": "t-settled",
        "status": {"state": "done", "answer": "found", "records": {}},
        "summary": "found it",
        "progress": 1.0,
        "next": []
    }))
    .unwrap();
    let settled = settle(&config, view.clone()).await.unwrap();
    assert_eq!(settled, view);
}
