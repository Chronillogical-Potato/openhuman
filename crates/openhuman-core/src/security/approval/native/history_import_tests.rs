use super::*;
use chrono::{Duration, Utc};

fn legacy(id: &str, decision: Option<&str>, executed: bool, age_minutes: i64) -> LegacyApproval {
    let created = Utc::now() - Duration::minutes(age_minutes);
    LegacyApproval {
        request_id: id.into(),
        tool_name: "composio".into(),
        agent_id: None,
        created_at: created,
        expires_at: Some(created + Duration::minutes(10)),
        decided_at: decision.map(|_| created + Duration::seconds(5)),
        decision: decision.map(str::to_owned),
        executed,
    }
}

#[test]
fn a_legacy_decision_maps_to_the_module_state_it_ended_in() {
    assert_eq!(
        state_of(&legacy("a", Some("approve_once"), false, 1)),
        ApprovalState::Allowed
    );
    assert_eq!(
        state_of(&legacy("a", Some("approve_always_for_tool"), true, 1)),
        ApprovalState::Executed
    );
    assert_eq!(
        state_of(&legacy("a", Some("deny"), false, 1)),
        ApprovalState::Denied
    );
    // A request an earlier launch left undecided can never be resumed.
    assert_eq!(
        state_of(&legacy("a", None, false, 1)),
        ApprovalState::Expired
    );
}

#[test]
fn the_legacy_record_carries_identity_without_the_arguments_or_summary() {
    let row = legacy("req-1", Some("deny"), false, 3);
    let record = legacy_record(&row);
    assert_eq!(record.request_id, "req-1");
    assert_eq!(record.caller.agent, "operator");
    assert_eq!(record.caller.call_id, "req-1");
    assert_eq!(record.fingerprint.len(), 64);
    assert_eq!(record.state, ApprovalState::Denied);
    assert_eq!(
        record.expires_at_ms,
        u64::try_from(row.expires_at.unwrap().timestamp_millis()).unwrap()
    );
    let json = serde_json::to_string(&record).unwrap();
    assert!(
        !json.contains("composio"),
        "the tool name is not sent: {json}"
    );
    // The same row always fingerprints the same, so a repeat run is recognized.
    assert_eq!(legacy_record(&row), record);
    let mut other = row.clone();
    other.request_id = "req-2".into();
    assert_ne!(legacy_record(&other).fingerprint, record.fingerprint);
}

#[test]
fn an_agent_scoped_row_keeps_its_agent_and_a_row_without_expiry_gets_the_default_window() {
    let mut row = legacy("req-1", Some("approve_once"), false, 3);
    row.agent_id = Some("agent-7".into());
    row.expires_at = None;
    let record = legacy_record(&row);
    assert_eq!(record.caller.agent, "agent-7");
    assert_eq!(
        record.expires_at_ms,
        u64::try_from(row.created_at.timestamp_millis()).unwrap() + DEFAULT_WINDOW_MS
    );
}

#[test]
fn only_the_most_recent_decided_rows_are_carried_over_oldest_first() {
    let mut history = LegacyHistory::default();
    history
        .approvals
        .push(legacy("undecided", None, false, 100_000));
    let total = HISTORY_LIMIT + 7;
    for index in 0..total {
        let age = i64::try_from(total - index).unwrap();
        history.approvals.push(legacy(
            &format!("r{index}"),
            Some("approve_once"),
            false,
            age,
        ));
    }
    let carried = recent_decided(&history);
    assert_eq!(carried.len(), HISTORY_LIMIT);
    assert_eq!(carried.first().unwrap().request_id, "r7");
    assert_eq!(
        carried.last().unwrap().request_id,
        format!("r{}", total - 1)
    );
    assert!(carried.iter().all(|row| row.request_id != "undecided"));
}

