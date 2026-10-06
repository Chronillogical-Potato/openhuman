//! Host adapter over the upstream abort-on-drop steering forwarder
//! (`tinyagents_harness::run_queue::SteeringForwarderGuard`, issue #4456).
//!
//! `run_turn_via_tinyagents_shared` bridges OpenHuman's session-owned
//! [`RunQueue`] into a running TinyAgents turn by spawning a poll loop that
//! drains queued **steer**/**collect** messages into the run's
//! `SteeringHandle`. The loop, the drop-safe cleanup, and the residual-steer
//! requeue are upstream; this module supplies the host seams:
//!
//! - [`QueuedMessage`] for [`QueuedTurn`] (text/id accessors and how a recovered
//!   steer is re-created),
//! - an event sink that publishes [`DomainEvent::RunQueueMessageDelivered`] /
//!   [`DomainEvent::RunQueueSteerRequeued`] on the event bus,
//! - the cleanup hook that deregisters a sub-agent's handle from the shared
//!   steering registry.

use std::sync::Arc;

use tinyagents_harness::ids::TaskId;
use tinyagents_harness::run_queue::{
    self, ForwardEvent, ForwardEventSink, QueuedMessage, RunQueue, SteeringForwarderGuard,
};
use tinyagents_harness::steering::SteeringHandle;

use crate::agent::queued_turn::{text_preview, QueuedTurn};
use crate::core::bus::BUS;
use crate::core::events::DomainEvent;

use super::host::steering::shared_steering_registry;

impl QueuedMessage for QueuedTurn {
    fn id(&self) -> &str {
        &self.id
    }

    fn text(&self) -> &str {
        &self.text
    }

    fn requeued(id: String, text: String, thread_label: &str, queued_at_ms: u64) -> Self {
        QueuedTurn {
            id,
            text,
            client_id: String::new(),
            thread_id: thread_label.to_string(),
            queued_at_ms,
            model_override: None,
            temperature: None,
            locale: None,
        }
    }
}

/// Publish forwarder notifications as `RunQueue*` domain events.
fn event_bus_sink() -> ForwardEventSink {
    Arc::new(|event| match event {
        ForwardEvent::Delivered {
            thread_label,
            mode,
            delivered,
            item_id,
            text,
        } => BUS.publish(DomainEvent::RunQueueMessageDelivered {
            thread_id: thread_label,
            mode: mode.to_string(),
            delivered,
            item_id,
            text_preview: text.as_deref().map(text_preview),
        }),
        ForwardEvent::Requeued {
            thread_label,
            requeued,
            item_id,
            text,
        } => BUS.publish(DomainEvent::RunQueueSteerRequeued {
            thread_id: thread_label,
            requeued,
            item_id,
            text_preview: text.as_deref().map(text_preview),
        }),
    })
}

/// Drain the queue's pending **steer** messages into `handle` (pre-run drain
/// and poll-loop body). See `tinyagents_harness::run_queue::forward_steers`.
pub(super) async fn forward_steers(
    queue: &RunQueue<QueuedTurn>,
    handle: &SteeringHandle,
    thread_label: &str,
) {
    run_queue::forward_steers(queue, handle, thread_label, &event_bus_sink()).await;
}

/// Drain the queue's pending **collect** messages into `handle`. See
/// `tinyagents_harness::run_queue::forward_collects`.
pub(super) async fn forward_collects(
    queue: &RunQueue<QueuedTurn>,
    handle: &SteeringHandle,
    thread_label: &str,
) {
    run_queue::forward_collects(queue, handle, thread_label, &event_bus_sink()).await;
}

/// Arm the abort-on-drop guard. `registry_task_id` is `Some` for sub-agent runs
/// (whose handle is registered in the shared steering registry and deregistered
/// on every exit path) and `None` for the interactive parent turn.
pub(super) fn arm_guard(
    handle: SteeringHandle,
    run_queue: Option<Arc<RunQueue<QueuedTurn>>>,
    registry_task_id: Option<TaskId>,
    thread_label: String,
) -> SteeringForwarderGuard<QueuedTurn> {
    let cleanup: Option<run_queue::ForwarderCleanup> = registry_task_id.map(|task_id| {
        Box::new(move || {
            shared_steering_registry().deregister(&task_id);
            tracing::debug!(
                task_id = task_id.as_str(),
                "[tinyagents] deregistered subagent steering handle (guard drop)"
            );
        }) as run_queue::ForwarderCleanup
    });
    SteeringForwarderGuard::new(handle, run_queue, cleanup, thread_label, event_bus_sink())
}

#[cfg(test)]
#[path = "steering_forwarder_tests.rs"]
mod tests;
