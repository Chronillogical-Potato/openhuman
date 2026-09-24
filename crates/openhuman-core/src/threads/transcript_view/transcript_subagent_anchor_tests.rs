use super::project::project_thread;
use super::tests::write_raw;
use super::types::DisplayItem;
use tempfile::TempDir;

#[test]
fn subagent_anchors_to_parent_turn_by_spawn_timestamp() {
    let dir = TempDir::new().unwrap();
    let root_stem = "800_orchestrator";
    let thread_id = "thr_anchor";
    let t1 = chrono::DateTime::from_timestamp(1_000_000, 0)
        .unwrap()
        .to_rfc3339();
    let t2 = chrono::DateTime::from_timestamp(2_000_000, 0)
        .unwrap()
        .to_rfc3339();
    let root_body = vec![
        r#"{"role":"user","content":"one","request_id":"req-1"}"#.to_string(),
        format!(
            r#"{{"role":"assistant","content":"a1","provider":"anthropic","model":"m","usage":{{"input":1,"output":1,"cached_input":0,"cost_usd":0.0}},"ts":"{t1}","iteration":1,"request_id":"req-1"}}"#
        ),
        r#"{"role":"user","content":"two","request_id":"req-2"}"#.to_string(),
        format!(
            r#"{{"role":"assistant","content":"a2","provider":"anthropic","model":"m","usage":{{"input":1,"output":1,"cached_input":0,"cost_usd":0.0}},"ts":"{t2}","iteration":1,"request_id":"req-2"}}"#
        ),
    ];
    let root_refs: Vec<&str> = root_body.iter().map(String::as_str).collect();
    write_raw(dir.path(), root_stem, thread_id, &root_refs);
    write_raw(
        dir.path(),
        &format!("{root_stem}__1000050_coder"),
        thread_id,
        &[r#"{"role":"assistant","content":"coder work"}"#],
    );
    write_raw(
        dir.path(),
        &format!("{root_stem}__2000050_planner"),
        thread_id,
        &[r#"{"role":"assistant","content":"planner work"}"#],
    );

    let mut anchors: Vec<(String, Option<String>)> = project_thread(dir.path(), thread_id)
        .expect("project thread")
        .items
        .iter()
        .filter_map(|item| match item {
            DisplayItem::Subagent {
                request_id, items, ..
            } => items.iter().find_map(|inner| match inner {
                DisplayItem::AssistantMessage { content, .. } => {
                    Some((content.clone(), request_id.clone()))
                }
                _ => None,
            }),
            _ => None,
        })
        .collect();
    anchors.sort();

    assert_eq!(
        anchors,
        vec![
            ("coder work".to_string(), Some("req-1".to_string())),
            ("planner work".to_string(), Some("req-2".to_string())),
        ]
    );
}
