//! The approval store on a configured storage backend, end to end through
//! its public functions.
//!
//! Its own test binary because it installs a backend into the process-wide
//! storage slot, which would reroute every other suite's approvals in a
//! shared process.

use std::sync::Arc;

use chrono::Utc;
use openhuman_core::config::Config;
use openhuman_core::security::approval::store;
use openhuman_core::security::approval::types::{ApprovalDecision, PendingApproval};

fn pending(id: &str) -> PendingApproval {
    PendingApproval {
        request_id: id.to_string(),
        tool_name: "shell".to_string(),
        action_summary: format!("run {id}"),
        args_redacted: serde_json::json!({ "cmd": "ls" }),
        created_at: Utc::now(),
        expires_at: None,
        source_context: None,
        tool_call_id: None,
    }
}

#[test]
fn a_configured_backend_holds_the_approvals_instead_of_approval_db() {
    let workspace = tempfile::tempdir().unwrap();
    let config = Config {
        workspace_dir: workspace.path().to_path_buf(),
        ..Config::default()
    };
    openhuman_core::storage::install(Arc::new(openhuman_core::storage::MemoryStorage::new()));

    store::insert_pending(&config, &pending("r1"), "session").unwrap();
    store::insert_flow_trust(&config, "flow-1", "shell").unwrap();
    assert_eq!(store::list_pending(&config).unwrap().len(), 1);
    assert!(store::is_flow_tool_trusted(&config, "flow-1", "shell").unwrap());
    let decided = store::decide(&config, "r1", ApprovalDecision::ApproveOnce).unwrap();
    assert!(decided.is_some());
    assert_eq!(
        store::get_decision(&config, "r1").unwrap(),
        Some(ApprovalDecision::ApproveOnce)
    );
    assert_eq!(store::list_recent_decisions(&config, 10).unwrap().len(), 1);
    assert_eq!(store::expire_stale(&config).unwrap(), 0);
    assert_eq!(store::purge_session(&config, "session").unwrap(), 0);

    assert!(
        !workspace
            .path()
            .join("approval")
            .join("approval.db")
            .exists(),
        "nothing was written to the classic database"
    );

    // Without a backend the classic database is back in use.
    assert!(openhuman_core::storage::clear());
    assert!(store::list_pending(&config).unwrap().is_empty());
    assert!(workspace
        .path()
        .join("approval")
        .join("approval.db")
        .exists());
}
