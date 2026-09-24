use super::*;

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
        &format!("{root_stem}__999950_coder"),
        thread_id,
        &[r#"{"role":"assistant","content":"coder work"}"#],
    );
    write_raw(
        dir.path(),
        &format!("{root_stem}__1000050_planner"),
        thread_id,
        &[r#"{"role":"assistant","content":"planner work"}"#],
    );

    let projected = project_thread(dir.path(), thread_id).expect("project thread");
    let mut anchors: Vec<(String, Option<String>)> = projected
        .items
        .iter()
        .filter_map(|item| match item {
            DisplayItem::Subagent {
                request_id, items, ..
            } => {
                let marker = items.iter().find_map(|inner| match inner {
                    DisplayItem::AssistantMessage { content, .. } => Some(content.clone()),
                    _ => None,
                })?;
                Some((marker, request_id.clone()))
            }
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

/// Exact correlation (#C1): when the run ledger records the spawning
/// `parentCallId` for this task, it wins over the timestamp/target-argument
/// heuristic — which would otherwise pick the first unclaimed
/// delegation-shaped call, regardless of which one actually spawned this
/// child.
#[test]
fn subagent_correlates_by_ledger_parent_call_id_over_the_heuristic() {
    let dir = TempDir::new().unwrap();
    let root_stem = "800_orch_exact";
    let thread_id = "thr_exact";
    let commit_ts = chrono::DateTime::from_timestamp(1_900_000, 0)
        .unwrap()
        .to_rfc3339();
    let root_body = vec![
        r#"{"role":"user","content":"do research","request_id":"req-1"}"#.to_string(),
        format!(
            r#"{{"role":"assistant","content":"","tool_calls":[{{"id":"call-decoy","name":"spawn_async_subagent","arguments":"{{}}"}},{{"id":"call-real","name":"spawn_async_subagent","arguments":"{{}}"}}],"iteration":1,"request_id":"req-1","ts":"{commit_ts}"}}"#
        ),
    ];
    let root_refs: Vec<&str> = root_body.iter().map(String::as_str).collect();
    write_raw(dir.path(), root_stem, thread_id, &root_refs);

    // Spawned at unix 2_000_000 — after the only turn's commit, so it
    // anchors to that turn either way; the heuristic would still pick the
    // first unclaimed `spawn_*`-shaped call (`call-decoy`) since neither
    // call names an agent. Only the exact ledger lookup can tell them apart.
    let child_stem = format!("{root_stem}__2000000_000000001_researcher");
    let child = transcript::resolve_keyed_transcript_path(dir.path(), &child_stem).unwrap();
    write_raw_at(
        &child,
        thread_id,
        &[r#"{"role":"assistant","content":"Bali is great."}"#],
    );
    // `write_raw_at` doesn't set `_meta.task_id`; patch it in directly so
    // `build_child` picks it up as the ledger correlation key.
    let raw = std::fs::read_to_string(&child).unwrap();
    let mut lines: Vec<String> = raw.lines().map(str::to_string).collect();
    let mut meta_json: serde_json::Value = serde_json::from_str(
        lines[0]
            .strip_prefix('{')
            .map(|_| lines[0].as_str())
            .unwrap(),
    )
    .unwrap();
    meta_json["_meta"]["task_id"] = serde_json::json!("sub-exact-1");
    meta_json["_meta"]["agent_id"] = serde_json::json!("researcher");
    lines[0] = meta_json.to_string();
    std::fs::write(&child, lines.join("\n") + "\n").unwrap();

    tinyagents_session::run_ledger::upsert_agent_run(
        dir.path(),
        tinyagents_session::run_ledger::AgentRunUpsert {
            id: "sub-exact-1".to_string(),
            kind: tinyagents_session::run_ledger::AgentRunKind::Subagent,
            parent_run_id: None,
            parent_thread_id: Some(thread_id.to_string()),
            agent_id: Some("researcher".to_string()),
            status: tinyagents_session::run_ledger::AgentRunStatus::Completed,
            prompt_ref: None,
            worker_thread_id: None,
            checkpoint_path: None,
            checkpoint: None,
            summary: None,
            error: None,
            metadata: serde_json::json!({ "parentCallId": "call-real" }),
            started_at: None,
            completed_at: None,
        },
    )
    .expect("seed run ledger row");

    let projected = project_thread(dir.path(), thread_id).expect("project thread");
    let subagent_call_id = projected.items.iter().find_map(|item| match item {
        DisplayItem::Subagent { call_id, .. } => Some(call_id.clone()),
        _ => None,
    });
    assert_eq!(
        subagent_call_id,
        Some(Some("call-real".to_string())),
        "exact ledger correlation must win over the first-unclaimed heuristic; items={:#?}",
        projected.items
    );
}
