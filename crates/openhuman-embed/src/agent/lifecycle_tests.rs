use super::*;
use std::sync::Arc;

fn removed(outcome: Result<(), CoreError>) -> bool {
    matches!(outcome, Err(CoreError::AgentRemoved { ref agent_id, .. }) if agent_id == "a")
}

#[tokio::test]
async fn a_live_agent_runs_its_turns() {
    let lifecycle = Lifecycle::new();
    let outcome = lifecycle.admit("a", "test", async { Ok(7) }).await;
    assert_eq!(outcome.unwrap(), 7);
    assert!(lifecycle.wait_idle(Duration::from_millis(10)).await);
}

#[tokio::test]
async fn a_removed_agent_refuses_new_turns() {
    let lifecycle = Lifecycle::new();
    lifecycle.mark_removed();
    let ran = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&ran);
    let outcome = lifecycle
        .admit("a", "test", async move {
            flag.store(true, Ordering::SeqCst);
            Ok(())
        })
        .await;
    assert!(removed(outcome));
    assert!(!ran.load(Ordering::SeqCst));
}

#[tokio::test]
async fn removal_ends_a_turn_in_flight_and_the_agent_goes_idle() {
    let lifecycle = Arc::new(Lifecycle::new());
    let running = Arc::clone(&lifecycle);
    let turn = tokio::spawn(async move {
        running
            .admit("a", "test", std::future::pending::<Result<(), CoreError>>())
            .await
    });
    while lifecycle.in_flight.load(Ordering::SeqCst) == 0 {
        tokio::task::yield_now().await;
    }
    assert!(!lifecycle.wait_idle(Duration::from_millis(10)).await);

    lifecycle.mark_removed();

    assert!(removed(turn.await.expect("turn task")));
    assert!(lifecycle.wait_idle(Duration::from_secs(1)).await);
}

#[test]
fn teardown_is_claimed_once() {
    let lifecycle = Lifecycle::new();
    assert!(lifecycle.begin_teardown());
    assert!(!lifecycle.begin_teardown());
}
