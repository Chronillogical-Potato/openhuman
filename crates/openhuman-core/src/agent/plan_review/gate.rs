//! `PlanReviewGate` — parks a live interactive turn on a plan the user must
//! review before execution.
//!
//! Flow (mirrors [`crate::security::approval::ApprovalGate`], in-memory):
//! 1. The orchestrator calls the `request_plan_review` tool after laying out a
//!    thread-scoped plan. The tool calls [`PlanReviewGate::request_review`].
//! 2. The gate registers a `oneshot::Sender` keyed by `request_id`, publishes
//!    [`DomainEvent::PlanReviewRequested`] (bridged to the `plan_review_request`
//!    socket event), and parks the turn on the receiver.
//! 3. The UI's `PlanReviewCard` calls `plan_review_decide` (RPC) →
//!    [`PlanReviewGate::decide`] → sends the resolution on the oneshot.
//! 4. The parked turn wakes with [`PlanReviewResolution`] and the tool returns
//!    a result that tells the agent to proceed / stop / revise.
//!
//! On TTL or a dropped sender the gate resolves to `Reject` — fail-closed, so a
//! plan never executes without an explicit approval.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::core::bus::BUS;
use crate::core::events::DomainEvent;

use super::types::PlanReviewResolution;

/// How long the gate parks a turn before timing out → `Reject`. Matches the
/// default approval TTL (10 min) — long enough for a human to read the plan.
const DEFAULT_PLAN_REVIEW_TTL: Duration = Duration::from_secs(60 * 10);

/// Snapshot of a parked plan review, captured at park time. Used to replay
/// the `plan_review_request` event to a socket that (re)joins the thread
/// room after the live emit — mirrors
/// [`crate::security::approval::PendingApproval`], but in-memory only (a
/// plan review parks a live turn; it has nothing to recover across a
/// restart).
#[derive(Debug, Clone)]
pub struct ParkedReview {
    pub request_id: String,
    pub thread_id: Option<String>,
    pub client_id: Option<String>,
    pub summary: String,
    pub steps: Vec<String>,
    /// The gated tool call's provider-assigned call id, when known.
    pub tool_call_id: Option<String>,
    /// RFC3339 expiry of this parked review.
    pub expires_at: Option<String>,
}

/// In-memory registry of parked plan reviews. Process-global singleton (see
/// [`global`]); no persistence — a parked interactive turn cannot resume
/// across a restart, so an orphaned review has nothing to recover.
pub struct PlanReviewGate {
    ttl: Duration,
    waiters: Mutex<HashMap<String, oneshot::Sender<PlanReviewResolution>>>,
    /// Newest parked `request_id` per thread, so a typed reply or a UI action
    /// that only knows the thread can resolve the latest review.
    thread_to_request: Mutex<HashMap<String, String>>,
    /// request_id → [`ParkedReview`] for every currently-parked review —
    /// consulted by `parked_review_for_thread` so a socket that (re)joins a
    /// thread room can be handed whatever is parked on it, the same
    /// reconciliation `ApprovalGate::parked_request_for_thread` performs.
    parked: Mutex<HashMap<String, ParkedReview>>,
}

impl PlanReviewGate {
    fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            waiters: Mutex::new(HashMap::new()),
            thread_to_request: Mutex::new(HashMap::new()),
            parked: Mutex::new(HashMap::new()),
        }
    }

    /// Park the current turn on a plan review and block until the user decides
    /// or the TTL elapses. `summary` is a one-line description; `steps` are the
    /// ordered plan items shown in the review card. `thread_id` / `client_id`
    /// route the surface to the originating chat (absent → no routable surface,
    /// so the park TTL-rejects). `tool_call_id` is the gated `request_plan_review`
    /// call's provider-assigned id, when known, so the UI can correlate the
    /// review card back to the exact timeline row.
    pub async fn request_review(
        &self,
        thread_id: Option<String>,
        client_id: Option<String>,
        summary: String,
        steps: Vec<String>,
        tool_call_id: Option<String>,
    ) -> PlanReviewResolution {
        let request_id = format!("plan-{}", Uuid::new_v4());
        let (tx, rx) = oneshot::channel();
        self.waiters.lock().insert(request_id.clone(), tx);
        if let Some(tid) = thread_id.clone() {
            self.thread_to_request
                .lock()
                .insert(tid, request_id.clone());
        }
        let expires_at = (chrono::Utc::now() + chrono::Duration::from_std(self.ttl).unwrap_or_default())
            .to_rfc3339();
        self.parked.lock().insert(
            request_id.clone(),
            ParkedReview {
                request_id: request_id.clone(),
                thread_id: thread_id.clone(),
                client_id: client_id.clone(),
                summary: summary.clone(),
                steps: steps.clone(),
                tool_call_id: tool_call_id.clone(),
                expires_at: Some(expires_at.clone()),
            },
        );

        // RAII cleanup: remove the waiter + thread + parked mappings on ANY
        // exit path, including when the parked future is cancelled/dropped
        // before `rx.await` completes (turn cancel, supervisor shutdown).
        // Without this, a cancelled review would leak entries that could be
        // re-decided against a dead turn or replayed as still-pending.
        let _guard = ParkGuard {
            gate: self,
            request_id: request_id.clone(),
            thread_id: thread_id.clone(),
        };

        tracing::info!(
            request_id = %request_id,
            thread_id = ?thread_id,
            steps = steps.len(),
            "[plan_review::gate] parking turn for plan review"
        );

        BUS.publish(DomainEvent::PlanReviewRequested {
            request_id: request_id.clone(),
            thread_id: thread_id.clone(),
            client_id: client_id.clone(),
            summary,
            steps,
            tool_call_id: tool_call_id.clone(),
            expires_at: Some(expires_at),
        });

        let (resolution, timed_out_or_dropped) = match tokio::time::timeout(self.ttl, rx).await {
            Ok(Ok(resolution)) => (resolution, false),
            // Sender dropped (decided elsewhere / shutdown) or TTL elapsed →
            // fail closed: never execute a plan the user didn't approve.
            Ok(Err(_)) | Err(_) => {
                tracing::warn!(
                    request_id = %request_id,
                    "[plan_review::gate] review unresolved (timeout/dropped) → reject"
                );
                (PlanReviewResolution::Reject, true)
            }
        };

        // `_guard` drops here on the normal path too (cleanup is idempotent with
        // `decide`, which already removed the waiter).
        BUS.publish(DomainEvent::PlanReviewDecided {
            request_id: request_id.clone(),
            decision: resolution.as_str().to_string(),
            thread_id,
            client_id,
            tool_call_id,
            resolution: timed_out_or_dropped.then(|| "expired".to_string()),
        });
        tracing::info!(
            request_id = %request_id,
            decision = resolution.as_str(),
            "[plan_review::gate] review resolved"
        );
        resolution
    }

    /// The parked review on `thread_id`, if any — lets a socket (re)joining a
    /// thread room be handed whatever review is still parked on it.
    pub fn parked_review_for_thread(&self, thread_id: &str) -> Option<ParkedReview> {
        let request_id = self.thread_to_request.lock().get(thread_id)?.clone();
        self.parked.lock().get(&request_id).cloned()
    }

    /// Resolve a parked review by `request_id`. Returns `true` when a waiter was
    /// woken; `false` when the id is unknown (already decided / expired).
    pub fn decide(&self, request_id: &str, resolution: PlanReviewResolution) -> bool {
        let sender = self.waiters.lock().remove(request_id);
        match sender {
            Some(tx) => tx.send(resolution).is_ok(),
            None => {
                tracing::debug!(
                    request_id = %request_id,
                    "[plan_review::gate] decide for unknown/expired request"
                );
                false
            }
        }
    }

    /// Resolve the newest parked review on `thread_id` (typed-reply / thread-
    /// scoped path). Returns `true` when a waiter was woken.
    pub fn decide_by_thread(&self, thread_id: &str, resolution: PlanReviewResolution) -> bool {
        let request_id = self.thread_to_request.lock().get(thread_id).cloned();
        match request_id {
            Some(id) => self.decide(&id, resolution),
            None => false,
        }
    }
}

/// Removes a parked review's registry entries on drop — covers both the normal
/// return and cancellation of the parked future.
struct ParkGuard<'a> {
    gate: &'a PlanReviewGate,
    request_id: String,
    thread_id: Option<String>,
}

impl Drop for ParkGuard<'_> {
    fn drop(&mut self) {
        self.gate.waiters.lock().remove(&self.request_id);
        self.gate.parked.lock().remove(&self.request_id);
        if let Some(tid) = &self.thread_id {
            let mut map = self.gate.thread_to_request.lock();
            if map.get(tid) == Some(&self.request_id) {
                map.remove(tid);
            }
        }
    }
}

/// Process-global plan-review gate.
pub fn global() -> &'static PlanReviewGate {
    static GATE: OnceLock<PlanReviewGate> = OnceLock::new();
    GATE.get_or_init(|| PlanReviewGate::new(DEFAULT_PLAN_REVIEW_TTL))
}

#[cfg(test)]
#[path = "gate_tests.rs"]
mod tests;
