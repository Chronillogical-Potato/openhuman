//! A stopped task is summarised without page content, recorded once per
//! state, and its full report is written inside the workspace.

use super::*;
use serde_json::{json, Value};

fn view(id: &str, status: Value, progress: f32) -> TaskView {
    serde_json::from_value(json!({
        "id": id,
        "status": status,
        "summary": "searching",
        "progress": progress,
        "next": []
    }))
    .unwrap()
}

fn failed() -> Value {
    json!({
        "state": "failed",
        "step": 2,
        "reason": "the last three actions changed nothing on screen",
        "hint": "",
        "recoverable": true
    })
}

fn report(view: &TaskView) -> TaskReport {
    serde_json::from_value(json!({
        "view": view,
        "steps": [
            {"path": "0", "kind": "browse", "text": "open the site", "outcome": "done",
             "turns": 0, "jev_calls": 0, "actions": [], "loops": [], "note": ""},
            {"path": "1", "kind": "enter", "text": "type Asha", "outcome": "done",
             "turns": 2, "jev_calls": 14, "actions": [{"action": "fill", "ok": true}],
             "loops": [], "note": ""},
            {"path": "2", "kind": "do", "text": "search for trains", "outcome": "failed",
             "turns": 3, "jev_calls": 21,
             "actions": [{"action": "click", "ok": true}, {"action": "click", "ok": false}],
             "loops": [], "note": "nothing changed"}
        ],
        "records": {},
        "artifacts": [],
        "learned": [],
        "trace": [],
        "rescues": [
            {"step": 2, "failure": "nothing changed", "reason": "timed out", "outcome": "gave_up"}
        ]
    }))
    .unwrap()
}

#[test]
fn a_task_that_stopped_short_is_summarised_with_counts_not_page_text() {
    let view = view("t-summary", failed(), 0.33);
    let summary = Summary::of(&view.status, &report(&view));
    assert_eq!(
        summary,
        Summary {
            steps: 3,
            jev_calls: 35,
            actions: 3,
            rescues: 1,
            recovered: 0,
            failed_step: Some(2),
            failed_kind: Some("do"),
        }
    );
}

#[test]
fn only_a_task_that_stopped_short_or_a_traced_settled_one_is_recorded() {
    let done = view(
        "t-done",
        json!({"state": "done", "answer": "found", "records": {}}),
        1.0,
    );
    let running = view("t-running", json!({"state": "running"}), 0.5);
    let human = view(
        "t-human",
        json!({"state": "needs_human", "reason": "log in"}),
        0.5,
    );
    let plan = view("t-plan", json!({"state": "needs_plan", "guide": "…"}), 0.0);
    let failed = view("t-failed", failed(), 0.3);

    assert!(worth_recording(&failed.status, false));
    assert!(worth_recording(&human.status, false));
    assert!(worth_recording(&plan.status, false));
    assert!(!worth_recording(&done.status, false));
    assert!(worth_recording(&done.status, true));
    assert!(!worth_recording(&running.status, true));
}

#[test]
fn the_same_stop_is_recorded_once() {
    let first = view("t-dedupe", failed(), 0.33);
    assert!(first_time(&first));
    assert!(
        !first_time(&first),
        "followed again, it is not recorded twice"
    );
    let later = view("t-dedupe", failed(), 0.66);
    assert!(first_time(&later), "a later stop of the same task is new");
}

#[test]
fn the_logged_reason_is_the_modules_own_line_cut_short() {
    let long = "x".repeat(500);
    let status: TaskStatus = serde_json::from_value(json!({
        "state": "failed", "step": null, "reason": long, "hint": "", "recoverable": false
    }))
    .unwrap();
    assert_eq!(reason(&status).chars().count(), REASON_CHARS);
    let plan: TaskStatus =
        serde_json::from_value(json!({"state": "needs_plan", "guide": "write a flow"})).unwrap();
    assert_eq!(reason(&plan), "no planner is configured");
    assert_eq!(reason(&TaskStatus::Cancelled), "");
}

#[test]
fn a_step_kind_outside_the_flow_grammar_is_logged_as_other() {
    assert_eq!(kind_name("DO"), "do");
    assert_eq!(kind_name("stop_before"), "stop_before");
    assert_eq!(kind_name("<b>Pay</b>"), "other");
}

#[test]
fn a_report_path_stays_in_the_workspace_and_names_the_task_safely() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.workspace_dir = dir.path().join("workspace");
    let path = report_path(&config, "t-1/../etc", 42);
    assert_eq!(
        path,
        config
            .workspace_dir
            .join("state")
            .join("computer")
            .join("tasks")
            .join("42-t-1etc.json")
    );
}

#[tokio::test]
async fn a_traced_report_is_written_whole_into_the_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.workspace_dir = dir.path().join("workspace");
    let view = view("t-write", failed(), 0.33);
    let report = report(&view);
    write(&config, &report).await;
    let tasks = super::super::computer_config::trace_dir(&config).join("tasks");
    let written = std::fs::read_dir(&tasks)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(written.len(), 1, "{written:?}");
    let name = written[0]
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert!(name.ends_with("-t-write.json"), "{name}");
    let back: TaskReport = serde_json::from_slice(&std::fs::read(&written[0]).unwrap()).unwrap();
    assert_eq!(back, report);
}
