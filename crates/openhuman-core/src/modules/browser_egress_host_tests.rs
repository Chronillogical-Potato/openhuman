use super::*;
use tinybus::{broker::Broker, transport::memory::MemoryBus, Connection};

fn target(policy_id: &str) -> BrowserEgressTarget {
    BrowserEgressTarget {
        policy_id: policy_id.into(),
        expected_generation: 1,
        session_id: "browser-session-test".into(),
        scheme: "https".into(),
        hostname: "8.8.8.8".into(),
        port: 443,
    }
}

async fn host_bus() -> (MemoryBus, Connection, tokio::task::JoinHandle<tinybus::Result<()>>) {
    let transport = MemoryBus::new();
    let broker = Broker::new();
    let task = broker.spawn(transport.clone());
    let host = Connection::connect(transport.connect().await.unwrap())
        .await
        .unwrap();
    install(&host).await.unwrap();
    (transport, host, task)
}

#[tokio::test]
async fn browser_egress_callback_rejects_unauthenticated_peers() {
    let (transport, _host, _broker) = host_bus().await;
    let caller = Connection::connect(transport.connect().await.unwrap())
        .await
        .unwrap();
    let proxy = caller
        .proxy(contract::INTERFACE, contract::OBJECT_PATH, contract::INTERFACE)
        .unwrap();
    let result: tinybus::Result<BrowserEgressGrant> = proxy
        .call(contract::methods::AUTHORIZE, (target("unregistered"),))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn browser_egress_callback_refuses_unknown_host_registered_scope() {
    let (transport, _host, _broker) = host_bus().await;
    let caller = Connection::connect(transport.connect().await.unwrap())
        .await
        .unwrap();
    caller
        .request_name(tinycomputer_bus::names::INTERFACE)
        .await
        .unwrap();
    let proxy = caller
        .proxy(contract::INTERFACE, contract::OBJECT_PATH, contract::INTERFACE)
        .unwrap();
    let result: tinybus::Result<BrowserEgressGrant> = proxy
        .call(contract::methods::AUTHORIZE, (target("unregistered"),))
        .await;
    assert!(result.is_err());
}
