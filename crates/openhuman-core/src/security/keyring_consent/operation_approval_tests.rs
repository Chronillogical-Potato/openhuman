use super::*;

#[tokio::test]
async fn approval_response_resolves_only_its_matching_request() {
    let mut registry = PendingConsentRegistry::default();
    let timeout = std::time::Duration::from_secs(30);
    let (first, first_waiter) = registry.register("read", "auth:first", timeout).unwrap();
    let (second, second_waiter) = registry.register("write", "auth:second", timeout).unwrap();

    assert_ne!(first.request_id, second.request_id);
    assert!(registry.resolve(&second.request_id, false));
    assert!(!registry.resolve(&second.request_id, true));

    drop(registry);
    assert!(first_waiter.await.is_err());
    assert!(!second_waiter.await.unwrap());
}

#[test]
fn pending_approval_capacity_is_bounded() {
    let mut registry = PendingConsentRegistry::default();
    for index in 0..MAX_PENDING_APPROVALS {
        registry
            .register(
                "read",
                &format!("auth:{index}"),
                std::time::Duration::from_secs(120),
            )
            .unwrap();
    }
    assert!(registry
        .register("read", "auth:overflow", std::time::Duration::from_secs(120))
        .is_err());
}

#[tokio::test]
async fn dropping_waiter_discards_later_resolution() {
    let mut registry = PendingConsentRegistry::default();
    let (pending, waiter) = registry
        .register(
            "delete",
            "auth:profile",
            std::time::Duration::from_secs(120),
        )
        .unwrap();
    drop(waiter);

    assert!(!registry.resolve(&pending.request_id, true));
}

#[tokio::test]
async fn timed_out_approval_is_removed_from_capacity() {
    let registry = Mutex::new(PendingConsentRegistry::default());
    let (pending, waiter) = registry
        .lock()
        .unwrap()
        .register("read", "auth:profile", std::time::Duration::from_millis(1))
        .unwrap();

    assert_eq!(
        await_decision(
            &registry,
            &pending.request_id,
            waiter,
            std::time::Duration::from_millis(1),
        )
        .await,
        None
    );
    assert!(registry.lock().unwrap().pending.is_empty());
}

#[test]
fn operation_outcome_is_correlated_and_consumed_once() {
    let call_id = uuid::Uuid::new_v4().to_string();
    call_outcomes()
        .lock()
        .unwrap()
        .insert(call_id.clone(), OperationOutcome::Denied);

    assert_eq!(
        take_operation_outcome(&call_id),
        Some(OperationOutcome::Denied)
    );
    assert_eq!(take_operation_outcome(&call_id), None);
}
