use super::*;
use tinyagents_harness::context::{RunConfig, RunContext};
use tinyagents_harness::runtime::AgentHarness;
use tinyagents_harness::testkit::ScriptedModel;

#[tokio::test]
async fn residual_collect_requeues_to_its_original_lane() {
    let queue = Arc::new(RunQueue::new());
    let handle = SteeringHandle::allow_all();
    let guard = SteeringForwarderGuard::new(
        handle.clone(),
        Some(queue.clone()),
        None,
        "thread-test".to_string(),
    );
    handle.send(SteeringCommand::InjectMessage(TaMessage::user(format!(
        "{COLLECT_PREFIX}recovered context"
    ))));
    drop(guard);
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if queue.status().await.collects == 1 {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("residual collect should be requeued before timeout");
    assert!(queue.drain(QueueLane::Steer).await.is_empty());
    let recovered = queue.drain(QueueLane::Collect).await;
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].text, "recovered context");
}

#[tokio::test]
async fn collect_reaches_the_next_model_boundary_as_additional_context() {
    let queue = Arc::new(RunQueue::new());
    queue
        .push(
            QueueLane::Collect,
            crate::agent::queued_turn::QueuedTurn {
                id: "queued-test".to_string(),
                text: "the deployment finished successfully".to_string(),
                client_id: "client-test".to_string(),
                thread_id: "thread-test".to_string(),
                queued_at_ms: 1,
                model_override: None,
                temperature: None,
                locale: None,
            },
        )
        .await;
    let handle = SteeringHandle::allow_all();
    forward_collects(&queue, &handle, "thread-test").await;
    let model = Arc::new(ScriptedModel::replies(vec!["done"]));
    let mut harness: AgentHarness<()> = AgentHarness::new();
    harness
        .register_model("scripted", model.clone())
        .set_default_model("scripted");
    harness
        .invoke_in_context(
            &(),
            RunContext::new(RunConfig::new("collect-boundary"), ()).with_steering(handle),
            vec![TaMessage::user("start")],
        )
        .await
        .expect("collect-context run should complete");
    let requests = model.requests();
    assert_eq!(requests.len(), 1, "one model boundary should be crossed");
    let collect = requests[0]
        .messages
        .iter()
        .find(|message| message.text().contains("deployment finished successfully"))
        .expect("the next model request should contain the collected context");
    assert!(matches!(collect, TaMessage::User(_)));
    assert_eq!(
        collect.text(),
        "[Additional context from user]: the deployment finished successfully"
    );
    assert!(
        !collect.text().starts_with(STEER_PREFIX),
        "collect must be context, not a steering instruction"
    );
}
