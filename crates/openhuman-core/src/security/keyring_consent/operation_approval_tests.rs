use super::*;

#[tokio::test]
async fn approval_response_resolves_only_its_matching_request() {
    let mut registry = PendingConsentRegistry::default();
    let (first, first_waiter) = registry.register("read", "auth:first").unwrap();
    let (second, second_waiter) = registry.register("write", "auth:second").unwrap();

    assert_ne!(first.request_id, second.request_id);
    assert!(registry.resolve(&second.request_id, false));
    assert!(!registry.resolve(&second.request_id, true));

    assert_eq!(first_waiter.await.unwrap(), None);
    assert_eq!(second_waiter.await.unwrap(), Some(false));
}

#[test]
fn pending_approval_capacity_is_bounded() {
    let mut registry = PendingConsentRegistry::default();
    for index in 0..MAX_PENDING_APPROVALS {
        registry
            .register("read", &format!("auth:{index}"))
            .unwrap();
    }
    assert!(registry.register("read", "auth:overflow").is_err());
}

#[tokio::test]
async fn dropping_waiter_discards_later_resolution() {
    let mut registry = PendingConsentRegistry::default();
    let (pending, waiter) = registry.register("delete", "auth:profile").unwrap();
    drop(waiter);

    assert!(!registry.resolve(&pending.request_id, true));
}
