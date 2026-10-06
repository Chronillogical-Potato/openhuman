use super::*;

#[test]
fn steering_registry_reexport_registers_task_handles() {
    let registry = shared_steering_registry();
    let handle = openhuman_steering_handle(SteeringRunClass::Background);
    let task_id = TaskId::new("task-steer");

    registry.register(task_id.clone(), handle);
    assert!(registry.get(&task_id).is_some());
    assert!(registry.deregister(&task_id).is_some());
    assert!(registry.get(&task_id).is_none());
}

#[test]
fn steering_policy_tightens_by_run_class() {
    // Interactive: only the two long-standing kinds; control-flow steering
    // stays closed so the user's live turn can't be cancelled/redirected
    // out from under it via a rogue steer.
    let interactive = openhuman_steering_handle(SteeringRunClass::Interactive);
    let policy = interactive.policy();
    assert!(policy.is_allowed(SteeringCommandKind::InjectMessage));
    assert!(policy.is_allowed(SteeringCommandKind::Pause));
    assert!(!policy.is_allowed(SteeringCommandKind::Cancel));
    assert!(!policy.is_allowed(SteeringCommandKind::Resume));
    assert!(!policy.is_allowed(SteeringCommandKind::Redirect));
    assert!(!policy.is_allowed(SteeringCommandKind::SetMetadata));

    // Background: additionally accepts graceful control-flow steering.
    let background = openhuman_steering_handle(SteeringRunClass::Background);
    let policy = background.policy();
    assert!(policy.is_allowed(SteeringCommandKind::InjectMessage));
    assert!(policy.is_allowed(SteeringCommandKind::Pause));
    assert!(policy.is_allowed(SteeringCommandKind::Cancel));
    assert!(policy.is_allowed(SteeringCommandKind::Resume));
    assert!(policy.is_allowed(SteeringCommandKind::Redirect));
    // Metadata replacement stays closed on every class until a control
    // surface owns it.
    assert!(!policy.is_allowed(SteeringCommandKind::SetMetadata));
}
