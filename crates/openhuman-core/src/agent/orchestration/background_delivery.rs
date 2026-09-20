//! Delivery subsystem for finished detached background sub-agents.
//!
//! Surfaces results recorded in [`super::background_completions`] back into the
//! originating chat as a single **system-injected** turn:
//!   * **idle-gated** — never mid-turn; defers while a user turn is in flight,
//!   * **debounced** — a burst of completions batches into one turn,
//!   * **batched** — every result ready at delivery time goes in one turn,
//!     each tagged by its sub-agent process id.
//!
//! The delivery turn is host-owned here rather than sharing the removed
//! task-board dispatcher. It persists its reply before announcing `chat_done`,
//! so a reconnect cannot lose a completed delegated result.

use std::collections::HashSet;
use std::future::Future;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;

use crate::agent::session_host::OpenHumanSessionHost;
use crate::core::bus::BUS;
use crate::core::events::DomainEvent;
use tinybus::EventHandler;
use tinybus::SubscriptionHandle;

use super::background_completions;

/// Coalesce completions landing within this window into one delivery turn.
const DEBOUNCE: Duration = Duration::from_secs(3);

/// Sessions with a user turn currently in flight — delivery defers while busy.
fn busy() -> &'static Mutex<HashSet<String>> {
    static BUSY: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    BUSY.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Sessions whose delivery turn is in flight — prevents two concurrent turns.
fn delivering() -> &'static Mutex<HashSet<String>> {
    static D: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    D.get_or_init(|| Mutex::new(HashSet::new()))
}

fn is_busy(session: &str) -> bool {
    busy()
        .lock()
        .expect("background_delivery busy poisoned")
        .contains(session)
}

struct BackgroundDeliveryHandler;

#[async_trait]
impl EventHandler<DomainEvent> for BackgroundDeliveryHandler {
    fn name(&self) -> &str {
        "agent_orchestration::background_delivery"
    }

    async fn handle(&self, event: &DomainEvent) {
        match event {
            DomainEvent::AgentTurnStarted { session_id, .. } => {
                busy()
                    .lock()
                    .expect("busy poisoned")
                    .insert(session_id.clone());
            }
            DomainEvent::AgentTurnCompleted { session_id, .. } => {
                busy().lock().expect("busy poisoned").remove(session_id);
                // A user turn just ended — drain anything that finished while it ran.
                schedule_delivery(session_id.clone(), Duration::from_millis(300));
            }
            DomainEvent::AgentError { session_id, .. } => {
                // A failed turn may not emit AgentTurnCompleted — clear busy so
                // delivery isn't stuck, then try to drain.
                busy().lock().expect("busy poisoned").remove(session_id);
                schedule_delivery(session_id.clone(), Duration::from_millis(300));
            }
            // Any subagent terminal state — completed, failed, or awaiting-user —
            // can arrive after the parent turn already went idle. Schedule a
            // debounced drain for all three so the pending result is delivered
            // promptly instead of sitting until some unrelated later turn. Only
            // `SubagentCompleted` used to trigger a drain, so a failure (or an
            // awaiting-user pause) after the parent turn went idle left the chat
            // stuck on the original "Accepted" response (#4896). Debounce so a
            // burst batches into a single turn.
            DomainEvent::SubagentCompleted { parent_session, .. }
            | DomainEvent::SubagentFailed { parent_session, .. }
            | DomainEvent::SubagentAwaitingUser { parent_session, .. } => {
                schedule_delivery(parent_session.clone(), DEBOUNCE);
            }
            _ => {}
        }
    }
}

/// Schedule a debounced delivery attempt for a session.
fn schedule_delivery(session: String, delay: Duration) {
    tokio::spawn(async move {
        tokio::time::sleep(delay).await;
        try_deliver(session).await;
    });
}

/// Snapshot the ready batch for a session **right now** (sync, testable): if the
/// session is idle, drain all ready results. Returns `None` (queue untouched)
/// when busy or nothing is pending. Headless filtering + delivery happen in the
/// caller, which can requeue the batch if the turn fails.
fn plan_delivery(session: &str) -> Option<Vec<background_completions::CompletedBackgroundAgent>> {
    if is_busy(session) {
        return None;
    }
    let batch = background_completions::take_pending(session);
    if batch.is_empty() {
        None
    } else {
        Some(batch)
    }
}

/// Re-queue a drained batch (after a failed delivery) so it retries on the next
/// idle drain rather than being lost.
fn requeue(session: &str, batch: Vec<background_completions::CompletedBackgroundAgent>) {
    for c in batch {
        // Preserve the terminal outcome on requeue so a failed / awaiting-input
        // result isn't downgraded to a success when a delivery turn fails (#4896).
        background_completions::record_outcome(
            session,
            c.task_id,
            c.agent_id,
            c.summary,
            c.parent_thread_id,
            c.outcome,
        );
    }
}

/// Drain + deliver pending completions for a session — if idle and not already
/// delivering. Batches everything ready at this instant into one system turn.
async fn try_deliver(session: String) {
    try_deliver_with(session, |thread_id, notice| async move {
        run_system_turn_on_thread(thread_id, notice).await
    })
    .await;
}

/// Delivery-loop core with an injected turn executor. Keeping the queue and
/// retry boundary independent from host execution lets tests prove a failed
/// durable append is requeued before any terminal announcement is observable.
async fn try_deliver_with<F, Fut>(session: String, mut deliver: F)
where
    F: FnMut(String, String) -> Fut,
    Fut: Future<Output = Result<String, String>>,
{
    if is_busy(&session) || !background_completions::has_pending(&session) {
        return;
    }
    // Claim the delivery slot — held for the WHOLE delivery (including the
    // awaited turn) so a concurrent completion can't start a second delivery
    // turn on the same thread. Skip if a delivery is already in flight.
    {
        let mut d = delivering().lock().expect("delivering poisoned");
        if !d.insert(session.clone()) {
            return;
        }
    }

    if let Some(batch) = plan_delivery(&session) {
        // A user turn can start (AgentTurnStarted -> busy) between plan_delivery's
        // gate and the awaited turn below. Re-check here so we don't stream a
        // *system* turn concurrently with a freshly-started user turn on the same
        // thread — requeue the drained batch and let the next idle drain retry.
        // (Narrows the window; a turn starting mid-await is still possible, but
        // both append into the thread and delivery is keyed to its own run id.)
        if is_busy(&session) {
            requeue(&session, batch);
            delivering()
                .lock()
                .expect("delivering poisoned")
                .remove(&session);
            return;
        }
        if let (Some(thread_id), Some(notice)) = (
            background_completions::batch_thread_id(&batch),
            background_completions::build_batched_notice(&batch),
        ) {
            log::info!(
                "[background_delivery] delivering {} batched background result(s) \
                 session={session} thread_id={thread_id}",
                batch.len()
            );
            if let Err(e) = deliver(thread_id, notice).await {
                log::warn!(
                    "[background_delivery] delivery turn failed session={session} error={e}"
                );
                requeue(&session, batch); // don't lose results on a failed turn
            }
        } else {
            log::warn!(
                "[background_delivery] dropping headless batch session={session} count={}",
                batch.len()
            );
        }
    }

    // Release the slot only AFTER the turn settles.
    delivering()
        .lock()
        .expect("delivering poisoned")
        .remove(&session);
}

/// Run one system-authored delivery turn on an existing conversation thread.
/// This is intentionally separate from task-board execution: it only delivers
/// a detached sub-agent result already produced by `background_completions`.
async fn run_system_turn_on_thread(thread_id: String, prompt: String) -> Result<String, String> {
    let config = crate::config::Config::load_or_init()
        .await
        .map_err(|error| format!("load config: {error:#}"))?;
    let run_id = format!("bgdeliver-{}", uuid::Uuid::new_v4());
    let mut host = OpenHumanSessionHost::from_config_for_agent(&config, "orchestrator")
        .map_err(|error| format!("build delivery host: {error:#}"))?;
    host.set_event_context(run_id.clone(), "background_delivery");
    host.set_thread_id(Some(&thread_id));
    // The hosted harness only retains streamed terminal text for an observed
    // turn. Background delivery has no UI progress consumer, so drain a local
    // sink solely to preserve the generated reply; otherwise a successful
    // provider response is replaced with the empty-turn fallback.
    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel(128);
    host.set_on_progress(Some(progress_tx));
    let progress_drain = tokio::spawn(async move {
        while progress_rx.recv().await.is_some() {}
    });
    let result = crate::agent::turn_origin::with_origin(
        crate::agent::turn_origin::AgentTurnOrigin::Cli,
        host.run_single(&prompt),
    )
    .await
    .map_err(|error| format!("{error:#}"));
    // The runtime session owns a cloned sender for the lifetime of `host`, so
    // explicitly end the local drain rather than waiting for channel closure.
    drop(host);
    progress_drain.abort();

    persist_then_announce(
        result,
        |content, success| {
            persist_delivery_reply(
                config.workspace_dir.clone(),
                &thread_id,
                &run_id,
                content.to_string(),
                success,
            )
            .map_err(|error| {
                tracing::warn!(%thread_id, %run_id, %error, "[background_delivery] could not persist reply before announcement");
                error
            })
        },
        |result| match result {
            Ok(response) => crate::web_chat::presentation::deliver_response_single_bubble(
                "system", &thread_id, &run_id, response, None,
            ),
            Err(error) => {
                crate::web_chat::publish_web_channel_event(crate::core::socketio::WebChannelEvent {
                    event: "chat_error".to_string(),
                    client_id: "system".to_string(),
                    thread_id: thread_id.clone(),
                    request_id: run_id.clone(),
                    message: Some(error.clone()),
                    error_type: Some("agent_error".to_string()),
                    ..Default::default()
                })
            }
        },
    )
}

/// Persist a terminal reply, then announce it. A persistence failure returns
/// before `announce` runs, allowing the outer delivery loop to requeue the
/// completed background batch without publishing a phantom terminal event.
fn persist_then_announce<P, A>(
    result: Result<String, String>,
    persist: P,
    announce: A,
) -> Result<String, String>
where
    P: FnOnce(&str, bool) -> Result<(), String>,
    A: FnOnce(&Result<String, String>),
{
    let (content, success) = match &result {
        Ok(text) => (text.trim().to_string(), true),
        Err(error) => (format!("Run failed: {error}"), false),
    };
    if !content.is_empty() {
        persist(&content, success)?;
    }
    announce(&result);
    result
}

/// Durably append a background-delivery reply before publishing its terminal
/// chat event. Callers must propagate failures: publishing `chat_done` or
/// `chat_error` without a stored row loses the result across reconnects.
fn persist_delivery_reply(
    workspace_dir: std::path::PathBuf,
    thread_id: &str,
    run_id: &str,
    content: String,
    success: bool,
) -> Result<(), String> {
    crate::memory::conversations::append_message(
        workspace_dir,
        thread_id,
        crate::memory::conversations::ConversationMessage {
            id: crate::memory::conversations::run_reply_message_id(run_id),
            content,
            message_type: "text".to_string(),
            extra_metadata: json!({
                "scope": "background_delivery",
                "success": success,
                "requestId": run_id,
            }),
            sender: "agent".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        },
    )
    .map(|_| ())
}

/// Register the delivery subscriber on the global event bus. Keeps the
/// subscription alive for the process lifetime. Idempotent.
pub(crate) fn register_background_delivery() {
    static HANDLE: OnceLock<Option<SubscriptionHandle>> = OnceLock::new();
    HANDLE.get_or_init(|| BUS.subscribe(Arc::new(BackgroundDeliveryHandler)));
}

#[cfg(test)]
#[path = "background_delivery_tests.rs"]
mod tests;
