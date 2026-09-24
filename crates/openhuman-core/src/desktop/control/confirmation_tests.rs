use super::*;

#[tokio::test]
async fn confirmation_requires_trusted_one_use_decision_for_same_goal() {
    super::super::ops::set_listener_is_loopback(true);
    let id = uuid::Uuid::new_v4().to_string();
    record(
        "TextEdit",
        "make a note",
        "thread-a",
        &json!({
            "stop":"confirmation_required", "confirmation_id":id,
            "pending":{"operation":"CLICK", "reason":"May save a file",
                "target":{"ref_id":"@s1:e2", "name":"Save", "role":"button"}}
        }),
    );
    assert!(take_approved(&id, Some("thread-a")).is_err());
    let listed = pending();
    let item = listed
        .iter()
        .find(|item| item.confirmation_id == id)
        .unwrap();
    assert_eq!(item.operation, "CLICK");
    assert_eq!(item.target_name.as_deref(), Some("Save"));
    assert_eq!(item.action_summary, "CLICK button 'Save' in TextEdit");
    let config = Config::default();
    let approved = confirm(&config, &id, true).await.unwrap();
    assert_eq!(approved["approve"], true);
    assert_eq!(
        take_approved(&id, None).unwrap_err(),
        "desktop confirmation requires a threaded agent run"
    );
    assert_eq!(
        take_approved(&id, Some("thread-b")).unwrap_err(),
        "desktop confirmation does not match this thread"
    );
    assert_eq!(
        take_approved(&id, Some("thread-a")).unwrap(),
        ("TextEdit".to_owned(), "make a note".to_owned())
    );
    assert!(take_approved(&id, Some("thread-a")).is_err());
}

#[test]
fn unnamed_target_cannot_be_approved() {
    super::super::ops::set_listener_is_loopback(true);
    let id = uuid::Uuid::new_v4().to_string();
    record(
        "TextEdit",
        "test",
        "thread-a",
        &json!({
            "stop":"confirmation_required", "confirmation_id":id,
            "pending":{"operation":"CLICK", "target":{"ref_id":"@s1:e2", "role":"button"}}
        }),
    );
    assert!(!pending().iter().any(|item| item.confirmation_id == id));
}
