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
            .as_deref(),
        Some("agent-7")
    );
    let local = session(None);
    assert_eq!(owner_of("owner-test-pending", Some(&local)).await, None);
}

#[tokio::test]
async fn a_remembered_owner_is_used_without_a_lookup() {
    remember("owner-test-cached", Some("agent-9".to_string()));
    assert_eq!(
        owner_of("owner-test-cached", None).await.as_deref(),
        Some("agent-9")
    );
    remember("owner-test-local", None);
    assert_eq!(owner_of("owner-test-local", None).await, None);
}

#[test]
fn the_agent_is_not_sent_over_the_wire_when_absent() {
    let json = serde_json::to_value(session(None)).unwrap();
    assert!(json.get("agent").is_none());
    let json = serde_json::to_value(session(Some("a"))).unwrap();
    assert_eq!(json["agent"], "a");
}
