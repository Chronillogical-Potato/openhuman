//! Public RPC-facing operations for the web channel: sending a chat message,
//! cancelling in-flight turns (primary and parallel, scoped or unscoped), and
//! inspecting/clearing a thread's run queue.

use serde_json::{json, Value};

use crate::core::socketio::WebChannelEvent;
use crate::rpc::RpcOutcome;

use super::super::event_bus::publish_web_channel_event;
use super::super::types::ChatRequestMetadata;
use super::parallel_turn::{cancel_parallel_turn_by_request_id, cancel_parallel_turns_for_thread};
use super::start_chat::start_chat;
use super::state::{cancel_in_flight_gracefully, cancel_should_target, key_for, IN_FLIGHT};

/// Cancel whatever turn is currently running on a thread (unscoped stop).
///
/// Back-compat entry point (Stop button / session teardown). For a cancel that
/// must only affect a specific turn — so a stale cancel can't kill a newer turn
/// on the same thread — use [`cancel_chat_scoped`] with the target `request_id`
/// (#4760).
pub async fn cancel_chat(client_id: &str, thread_id: &str) -> Result<Option<String>, String> {
    cancel_chat_scoped(client_id, thread_id, None).await
}

/// Cancel the in-flight turn(s) for a thread.
///
/// When `request_id` is `Some`, the cancel is **scoped**: it only tears down the
/// primary turn if that exact request is still running (and only the matching
/// parallel turn), so a stale cancel for a superseded request can't kill the
/// newer turn that replaced it (#4760). When `request_id` is `None`, it stops
/// whatever is running on the thread (primary + every parallel) — the "stop
/// everything" behaviour used by session teardown / a Stop button.
pub async fn cancel_chat_scoped(
    client_id: &str,
    thread_id: &str,
    request_id: Option<&str>,
) -> Result<Option<String>, String> {
    Ok(cancel_chat_inner(client_id, thread_id, request_id)
        .await?
        .request_id)
}

/// What one cancel tore down.
struct CancelOutcome {
    /// The primary or parallel turn that was cancelled, if any.
    request_id: Option<String>,
    /// Detached sub-agents stopped along with the turn (unscoped stops only).
    subagents_cancelled: usize,
}

async fn cancel_chat_inner(
    client_id: &str,
    thread_id: &str,
    request_id: Option<&str>,
) -> Result<CancelOutcome, String> {
    let client_id = client_id.trim();
    let thread_id = thread_id.trim();

    if client_id.is_empty() {
        return Err("client_id is required".to_string());
    }
    if thread_id.is_empty() {
        return Err("thread_id is required".to_string());
    }

    let map_key = key_for(thread_id);
    let mut removed_request_id: Option<String> = None;

    {
        let mut in_flight = IN_FLIGHT.lock().await;
        // #4760: only tear down the primary turn when the cancel is unscoped OR
        // targets exactly the request that is running. A stale cancel for an
        // already-superseded request must be a no-op so the newer turn lives.
        let should_cancel_primary = in_flight
            .get(&map_key)
            .map(|entry| cancel_should_target(request_id, &entry.request_id))
            .unwrap_or(false);
        if should_cancel_primary {
            if let Some(existing) = in_flight.remove(&map_key) {
                removed_request_id = Some(cancel_in_flight_gracefully(existing));
            }
        } else if let Some(rid) = request_id {
            log::info!(
                "[web-channel] ignoring stale cancel request_id={} for thread_id={} — current in-flight is {:?}; newer turn preserved",
                rid,
                thread_id,
                in_flight.get(&map_key).map(|e| e.request_id.as_str())
            );
        }
    }

    // Also tear down concurrent parallel (forked) turns. A scoped cancel targets
    // only the named parallel turn (if it is one); an unscoped cancel/stop
    // covers every parallel turn on the thread, not just the primary one.
    let cancelled_parallel = match request_id {
        Some(rid) => cancel_parallel_turn_by_request_id(thread_id, rid).await,
        None => cancel_parallel_turns_for_thread(thread_id).await,
    };

    // #4760: a scoped cancel that matched only a parallel (forked) turn — not the
    // primary — still genuinely tore a turn down and emitted its cancelled event.
    // Surface that id so `channel_web_cancel` reports `cancelled: true` with the
    // right request_id instead of misreporting a no-op just because the primary
    // turn wasn't the one cancelled.
    let cancelled_any = removed_request_id
        .clone()
        .or_else(|| cancelled_parallel.first().cloned());

    // An unscoped stop also halts the thread's detached work. Async sub-agents
    // (`spawn_async_subagent`) run on their own tasks and deliberately drop the
    // spawning turn's cancellation, so tearing the turn down leaves them
    // running — and when one finishes, background delivery starts a fresh
    // system turn on the thread, which reads as "Stop did nothing". Abort them
    // first, then drop anything already queued for delivery, so no result lands
    // in the gap. A scoped cancel names one turn and leaves the rest alone.
    let subagents_cancelled = if request_id.is_none() {
        // Gate completion recording before aborting: Tokio abort is
        // cooperative, so a child already finishing can otherwise enqueue in
        // the gap between abort and the queue sweep.
        let discarded =
            crate::agent::orchestration::background_completions::discard_pending_for_thread(
                thread_id,
            );
        let stopped = crate::agent::orchestration::running_subagents::stop_for_thread(thread_id);
        crate::agent::orchestration::background_completions::finish_stop_for_thread(
            thread_id, &stopped,
        );
        log::info!(
            "[web-channel] stop thread_id={} turn={:?} parallel={} subagents_cancelled={} completions_discarded={}",
            thread_id,
            removed_request_id,
            cancelled_parallel.len(),
            stopped.len(),
            discarded
        );
        stopped.len()
    } else {
        0
    };

    // Emit a cancelled chat_error for each cancelled turn (primary + parallels)
    // so every interleaved branch's UI is resolved. `chat_cancelled` is the new,
    // purpose-built terminal event (structured `cancel_reason`, no
    // `error_type` string to parse); `chat_error{error_type:"cancelled"}` is
    // kept alongside it for one release so an older frontend build still
    // resolves the turn.
    for request_id in removed_request_id.into_iter().chain(cancelled_parallel) {
        publish_web_channel_event(WebChannelEvent {
            event: "chat_error".to_string(),
            client_id: client_id.to_string(),
            thread_id: thread_id.to_string(),
            request_id: request_id.clone(),
            message: Some("Cancelled".to_string()),
            error_type: Some("cancelled".to_string()),
            ..Default::default()
        });
        publish_web_channel_event(WebChannelEvent {
            event: "chat_cancelled".to_string(),
            client_id: client_id.to_string(),
            thread_id: thread_id.to_string(),
            request_id,
            cancel_reason: Some("user_stop".to_string()),
            ..Default::default()
        });
    }

    Ok(CancelOutcome {
        request_id: cancelled_any,
        subagents_cancelled,
    })
}

pub async fn channel_web_chat(
    client_id: &str,
    thread_id: &str,
    message: &str,
    model_override: Option<String>,
    temperature: Option<f64>,
    locale: Option<String>,
    queue_mode: Option<String>,
    metadata: ChatRequestMetadata,
) -> Result<RpcOutcome<Value>, String> {
    let result = start_chat(
        client_id,
        thread_id,
        message,
        model_override,
        temperature,
        locale,
        queue_mode,
        metadata,
    )
    .await?;

    if let Ok(parsed) = serde_json::from_str::<Value>(&result) {
        return Ok(RpcOutcome::single_log(parsed, "web channel message queued"));
    }

    Ok(RpcOutcome::single_log(
        json!({
            "accepted": true,
            "client_id": client_id.trim(),
            "thread_id": thread_id.trim(),
            "request_id": result,
        }),
        "web channel request accepted",
    ))
}

/// Render one snapshotted queue item as the wire shape `web_queue_status` and
/// `queue_item_*` socket events share: `{ id, lane, text_preview }`.
fn queue_item_json(lane: tinyagents_harness::run_queue::QueueLane, item: &crate::agent::queued_turn::QueuedTurn) -> Value {
    json!({
        "id": item.id,
        "lane": lane.as_str(),
        "text_preview": crate::agent::queued_turn::text_preview(&item.text),
    })
}

pub async fn channel_web_queue_status(thread_id: &str) -> Result<RpcOutcome<Value>, String> {
    let map_key = key_for(thread_id);
    let in_flight = IN_FLIGHT.lock().await;
    if let Some(entry) = in_flight.get(&map_key) {
        let status = entry.run_queue.status().await;
        let items: Vec<Value> = entry
            .run_queue
            .snapshot()
            .await
            .iter()
            .map(|(lane, item)| queue_item_json(*lane, item))
            .collect();
        Ok(RpcOutcome::single_log(
            json!({
                "thread_id": thread_id.trim(),
                "active": true,
                "request_id": entry.request_id,
                "steers": status.steers,
                "followups": status.followups,
                "collects": status.collects,
                "total": status.total,
                "items": items,
            }),
            "queue status retrieved",
        ))
    } else {
        Ok(RpcOutcome::single_log(
            json!({
                "thread_id": thread_id.trim(),
                "active": false,
                "steers": 0,
                "followups": 0,
                "collects": 0,
                "total": 0,
                "items": Vec::<Value>::new(),
            }),
            "no active turn for thread",
        ))
    }
}

/// `channel.web_queue_remove` — retract one specific queued item (e.g. the
/// user deleted a queued message from the composer's queue UI) without
/// touching the rest of the queue. Emits `queue_item_removed` on an actual
/// removal; a no-op removal (unknown id, or no active turn) is silently
/// `removed: false` — the item is already gone either way.
pub async fn channel_web_queue_remove(
    client_id: &str,
    thread_id: &str,
    item_id: &str,
) -> Result<RpcOutcome<Value>, String> {
    let client_id = client_id.trim();
    let thread_id = thread_id.trim();
    let item_id = item_id.trim();
    if item_id.is_empty() {
        return Err("item_id is required".to_string());
    }
    let map_key = key_for(thread_id);
    let in_flight = IN_FLIGHT.lock().await;
    let Some(entry) = in_flight.get(&map_key) else {
        return Ok(RpcOutcome::single_log(
            json!({
                "thread_id": thread_id,
                "item_id": item_id,
                "removed": false,
            }),
            "no active turn for thread",
        ));
    };
    let removed = entry.run_queue.remove_where(|item| item.id == item_id).await;
    drop(in_flight);
    if removed > 0 {
        log::info!(
            "[web-channel] removed queued item thread_id={thread_id} item_id={item_id}"
        );
        publish_web_channel_event(WebChannelEvent {
            event: "queue_item_removed".to_string(),
            client_id: client_id.to_string(),
            thread_id: thread_id.to_string(),
            queue_item: Some(crate::core::socketio::QueueItemPayload {
                id: item_id.to_string(),
                lane: None,
                text_preview: None,
            }),
            ..Default::default()
        });
    }
    Ok(RpcOutcome::single_log(
        json!({
            "thread_id": thread_id,
            "item_id": item_id,
            "removed": removed > 0,
        }),
        "queue item remove processed",
    ))
}

pub async fn channel_web_queue_clear(thread_id: &str) -> Result<RpcOutcome<Value>, String> {
    let map_key = key_for(thread_id);
    let in_flight = IN_FLIGHT.lock().await;
    if let Some(entry) = in_flight.get(&map_key) {
        let dropped = entry.run_queue.clear().await;
        log::info!(
            "[web-channel] cleared queue thread_id={} dropped={}",
            thread_id,
            dropped
        );
        Ok(RpcOutcome::single_log(
            json!({
                "thread_id": thread_id.trim(),
                "cleared": true,
                "dropped": dropped,
            }),
            "queue cleared",
        ))
    } else {
        Ok(RpcOutcome::single_log(
            json!({
                "thread_id": thread_id.trim(),
                "cleared": false,
                "dropped": 0,
            }),
            "no active turn for thread",
        ))
    }
}

pub async fn channel_web_cancel(
    client_id: &str,
    thread_id: &str,
    request_id: Option<&str>,
) -> Result<RpcOutcome<Value>, String> {
    let outcome = cancel_chat_inner(client_id, thread_id, request_id).await?;

    // `request_id` is set only when a turn was torn down, and only then does a
    // `cancelled` chat_error follow. A client that sees `request_id: null` knows
    // no terminal event is coming and must settle its own running state.
    let cancelled = outcome.request_id.is_some() || outcome.subagents_cancelled > 0;

    Ok(RpcOutcome::single_log(
        json!({
            "cancelled": cancelled,
            "client_id": client_id.trim(),
            "thread_id": thread_id.trim(),
            "request_id": outcome.request_id,
            "subagents_cancelled": outcome.subagents_cancelled,
        }),
        "web channel cancellation processed",
    ))
}
