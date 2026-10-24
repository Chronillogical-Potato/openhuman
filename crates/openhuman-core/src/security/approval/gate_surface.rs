//! User-facing notification surfaces for parked approval requests.

use super::ApprovalDecision;

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Surfaces a flow-origin park as a `CoreNotification` (category `Agents`,
/// `kind: "flow-gate-approval"`) with three action buttons matching the
/// [`ApprovalDecision`] variants a flow-scoped approval accepts:
/// `approve_once` / `approve_always_for_flow` / `deny`. Each action's payload
/// carries the same `{kind, request_id, flow_id, tool_name, summary}` shape
/// (plus `run_id`, additive) so the frontend can dispatch straight to
/// `approval_decide` without a second round-trip to fetch the pending row.
///
/// Mirrors `flows::ops::notify_pending_approval` (the tinyflows-native
/// per-node HITL gate's notification) but is a distinct surface: this one
/// fires from the *tool-call* `ApprovalGate`, not the graph's own
/// `require_approval` gate node.
/// `workspace` is the handle and revision of the workspace the flow parked in.
/// This publisher bypasses the notification bridge that stamps workspace
/// identity, and the approval gate itself is process-wide — so without the
/// binding, the banner stays actionable after a workspace switch and approves
/// another workspace's pending call from inside the one the user moved to
/// (#5966). `None` only when the workspace could not be resolved; the caller
/// says why that fails open.
pub(super) fn publish_flow_gate_notification(
    request_id: &str,
    flow_id: &str,
    run_id: &str,
    tool_name: &str,
    summary: &str,
    workspace: Option<(String, u64)>,
) {
    use crate::desktop::notifications::bus::publish_core_notification;
    use crate::desktop::notifications::types::{
        CoreNotificationAction, CoreNotificationCategory, CoreNotificationEvent,
    };

    const KIND: &str = "flow-gate-approval";
    let base_payload = |action: ApprovalDecision| {
        serde_json::json!({
            "kind": KIND,
            "request_id": request_id,
            "flow_id": flow_id,
            "run_id": run_id,
            "tool_name": tool_name,
            "summary": summary,
            "decision": action.as_str(),
        })
    };

    publish_core_notification(CoreNotificationEvent {
        id: format!("{KIND}:{request_id}"),
        category: CoreNotificationCategory::Agents,
        title: "Workflow needs approval".to_string(),
        body: format!("\"{tool_name}\" — {summary}"),
        deep_link: None,
        timestamp_ms: now_ms(),
        actions: Some(vec![
            CoreNotificationAction {
                action_id: "approve_once".to_string(),
                label: "Approve once".to_string(),
                payload: Some(base_payload(ApprovalDecision::ApproveOnce)),
            },
            CoreNotificationAction {
                action_id: "approve_always_for_flow".to_string(),
                label: "Always allow for this workflow".to_string(),
                payload: Some(base_payload(ApprovalDecision::ApproveAlwaysForFlow)),
            },
            CoreNotificationAction {
                action_id: "deny".to_string(),
                label: "Deny".to_string(),
                payload: Some(base_payload(ApprovalDecision::Deny)),
            },
        ]),
        workspace: workspace.as_ref().map(|(handle, _)| handle.clone()),
        workspace_revision: workspace.map(|(_, revision)| revision),
    });
}
