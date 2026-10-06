use super::*;
use tinyagents_harness::run_queue::QueueLane;

#[test]
fn recovered_steer_becomes_a_queued_turn_on_the_forwarders_thread() {
    let turn = QueuedTurn::requeued("id-1".into(), "text".into(), "thread-9", 42);
    assert_eq!(turn.id(), "id-1");
    assert_eq!(turn.text(), "text");
    assert_eq!(turn.thread_id, "thread-9");
    assert_eq!(turn.queued_at_ms, 42);
    assert!(turn.client_id.is_empty());
    assert!(turn.model_override.is_none() && turn.temperature.is_none() && turn.locale.is_none());
}

#[tokio::test]
async fn forwarders_drain_their_lane_into_the_steering_handle() {
    let queue = RunQueue::<QueuedTurn>::new();
    queue
        .push(
            QueueLane::Steer,
            QueuedTurn::requeued("s".into(), "go left".into(), "t", 1),
        )
        .await;
    queue
        .push(
            QueueLane::Collect,
            QueuedTurn::requeued("c".into(), "fyi".into(), "t", 1),
        )
        .await;
    let handle = SteeringHandle::allow_all();
    forward_steers(&queue, &handle, "t").await;
    forward_collects(&queue, &handle, "t").await;
    assert_eq!(handle.drain().len(), 2);
    assert_eq!(queue.status().await.total, 0);
}

#[tokio::test]
async fn arm_guard_deregisters_the_subagent_handle_on_drop() {
    let task_id = TaskId::new("guard-task");
    let handle = SteeringHandle::allow_all();
    shared_steering_registry().register(task_id.clone(), handle.clone());
    let guard = arm_guard(handle, None, Some(task_id.clone()), "t".into());
    drop(guard);
    assert!(
        shared_steering_registry().get(&task_id).is_none(),
        "the guard must deregister the sub-agent handle"
    );
}
