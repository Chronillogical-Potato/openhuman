use super::*;

fn session(agent: Option<&str>) -> PairingSession {
    PairingSession {
        channel_id: "owner-test".to_string(),
        pairing_token: "token".to_string(),
        core_pubkey: "pk".to_string(),
        rpc_url: None,
        expires_at: "2099-01-01T00:00:00Z".to_string(),
        agent: agent.map(str::to_string),
    }
}

#[tokio::test]
async fn a_pending_pairing_names_its_agent() {
    let pending = session(Some("agent-7"));
    assert_eq!(
        owner_of("owner-test-pending", Some(&pending))
            .await
            .unwrap()
            .as_deref(),
        Some("agent-7")
    );
    let local = session(None);
    assert_eq!(owner_of("owner-test-pending", Some(&local)).await, Ok(None));
}

#[tokio::test]
async fn a_remembered_owner_is_used_without_a_lookup() {
    remember("owner-test-cached", Some("agent-9".to_string()));
    assert_eq!(
        owner_of("owner-test-cached", None)
            .await
            .unwrap()
            .as_deref(),
        Some("agent-9")
    );
    remember("owner-test-local", None);
    assert_eq!(owner_of("owner-test-local", None).await, Ok(None));
}

#[test]
fn the_agent_is_not_sent_over_the_wire_when_absent() {
    let json = serde_json::to_value(session(None)).unwrap();
    assert!(json.get("agent").is_none());
    let json = serde_json::to_value(session(Some("a"))).unwrap();
    assert_eq!(json["agent"], "a");
}

fn scoped_config(dir: &std::path::Path) -> crate::config::Config {
    crate::config::Config {
        workspace_dir: dir.join("workspace"),
        config_path: dir.join("config.toml"),
        ..Default::default()
    }
}

#[tokio::test]
async fn a_device_found_in_the_local_scope_belongs_to_local_and_is_remembered() {
    let tmp = tempfile::tempdir().unwrap();
    let config = scoped_config(tmp.path());
    super::super::store::insert_device(&config, "owner-test-found", "label", "pk", "hash").unwrap();
    let context = crate::core::runtime::CoreContext::for_test_with_config(
        crate::core::runtime::DomainSet::full(),
        config,
    );
    let owner =
        crate::core::runtime::CoreContext::scope(context, owner_of("owner-test-found", None)).await;
    assert_eq!(owner, Ok(None));
    assert_eq!(cached("owner-test-found"), Some(None));
}

#[tokio::test]
async fn a_channel_no_scope_knows_is_local_and_not_remembered() {
    let tmp = tempfile::tempdir().unwrap();
    let context = crate::core::runtime::CoreContext::for_test_with_config(
        crate::core::runtime::DomainSet::full(),
        scoped_config(tmp.path()),
    );
    let owner =
        crate::core::runtime::CoreContext::scope(context, owner_of("owner-test-missing", None))
            .await;
    assert_eq!(owner, Ok(None));
    assert_eq!(cached("owner-test-missing"), None);
}

#[test]
fn the_first_scope_holding_the_device_owns_it() {
    let found = decide(vec![
        (None, Ok(false)),
        (Some("a".into()), Err("down".into())),
        (Some("b".into()), Ok(true)),
    ]);
    assert_eq!(found, Ok(Some(Some("b".to_string()))));
    assert_eq!(decide(vec![(None, Ok(true))]), Ok(Some(None)));
}

#[test]
fn a_failed_lookup_without_a_match_fails_closed() {
    let failed = decide(vec![
        (None, Ok(false)),
        (Some("a".into()), Err("down".into())),
    ]);
    assert_eq!(
        failed,
        Err(OwnerLookupFailed {
            agent: Some("a".to_string()),
            error: "down".to_string(),
        })
    );
    assert_eq!(decide(vec![(None, Ok(false))]), Ok(None));
    assert_eq!(decide(Vec::new()), Ok(None));
}
