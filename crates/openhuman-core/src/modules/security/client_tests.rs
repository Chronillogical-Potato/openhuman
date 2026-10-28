use super::*;
use std::sync::{Arc, Mutex};
use tinybus::{broker::Broker, transport::memory::MemoryBus, Connection, ObjectPath};

#[derive(Clone, Default)]
struct DeviceFixture {
    pairing: Arc<Mutex<Option<tinysecurity_bus::DevicePairingRequest2>>>,
    frame: Arc<Mutex<Option<tinysecurity_bus::DeviceFrameRequest2>>>,
}

#[tinybus::interface(name = "ai.tinyhumans.tinysecurity.Policy")]
impl DeviceFixture {
    #[tinybus(confidential)]
    async fn device_pairing2(
        &self,
        request: tinysecurity_bus::DevicePairingRequest2,
    ) -> tinybus::Result<tinysecurity_bus::DevicePairingResponse2> {
        *self.pairing.lock().unwrap() = Some(request);
        Ok(tinysecurity_bus::DevicePairingResponse2 {
            receipt: None,
            pairing_id: None,
            session_id: None,
            core_public_key: None,
            device_public_key: None,
            frame: vec![3],
            expires_at_ms: 0,
        })
    }

    #[tinybus(confidential)]
    async fn device_frame2(
        &self,
        request: tinysecurity_bus::DeviceFrameRequest2,
    ) -> tinybus::Result<tinysecurity_bus::DeviceFrameResponse2> {
        *self.frame.lock().unwrap() = Some(request);
        Ok(tinysecurity_bus::DeviceFrameResponse2 { bytes: vec![2] })
    }
}

#[tokio::test]
async fn device_pairing_and_frame_calls_reject_unattested_recipients() {
    let bus = MemoryBus::new();
    Broker::new().spawn(bus.clone());
    let service = Connection::connect(bus.connect().await.unwrap())
        .await
        .unwrap();
    let caller = Connection::connect(bus.connect().await.unwrap())
        .await
        .unwrap();
    let fixture = DeviceFixture::default();
    service
        .serve_at(
            ObjectPath::new(tinysecurity_bus::names::OBJECT_PATH).unwrap(),
            fixture.clone(),
        )
        .await
        .unwrap();
    service.request_name("test.TinySecurity").await.unwrap();
    let proxy = caller
        .proxy(
            "test.TinySecurity",
            tinysecurity_bus::names::OBJECT_PATH,
            tinysecurity_bus::names::INTERFACE,
        )
        .unwrap();
    let object = crate::modules::ops::fixture_client(proxy)
        .object_client(
            super::super::MODULE_ID,
            tinysecurity_bus::names::OBJECT_PATH,
        )
        .await
        .unwrap();
    let client = Client::from_object(object);

    let pairing = tinysecurity_bus::DevicePairingRequest2 {
        call_id: "host-call".into(),
        operation: tinysecurity_bus::DevicePairingOperation2::Begin {
            host_event_id: "event".into(),
        },
    };
    let raw_proxy = caller
        .proxy(
            "test.TinySecurity",
            tinysecurity_bus::names::OBJECT_PATH,
            tinysecurity_bus::names::INTERFACE,
        )
        .unwrap();
    let raw: Result<tinysecurity_bus::DevicePairingResponse2, tinybus::Error> = raw_proxy
        .call_confidential(
            tinysecurity_bus::names::methods::DEVICE_PAIRING2,
            (pairing.clone(),),
        )
        .await;
    assert!(matches!(
        raw,
        Err(tinybus::Error::MethodFailed { name, .. }) if name == tinybus::Error::NOT_ATTESTED
    ));
    assert!(matches!(
        client.device_pairing2(pairing).await,
        Err(SecurityError::Unavailable(_))
    ));
    assert!(fixture.pairing.lock().unwrap().is_none());

    let frame = tinysecurity_bus::DeviceFrameRequest2 {
        scope_id: "scope".into(),
        handle: "opaque-session".into(),
        call_id: "frame-call".into(),
        operation: tinysecurity_bus::DeviceFrameOperation2::Open { frame: vec![7] },
    };
    assert!(matches!(
        client.device_frame2(frame).await,
        Err(SecurityError::Unavailable(_))
    ));
    assert!(fixture.frame.lock().unwrap().is_none());
}

/// The released module refuses private and metadata IP literals before
/// resolving or opening a socket. The test performs no network operation.
#[tokio::test]
#[ignore = "loads the pinned TinySecurity release to check private browser destinations"]
async fn native_fixture_denies_private_browser_dial() {
    let scratch = tempfile::tempdir().expect("scratch workspace");
    std::env::set_var("OPENHUMAN_WORKSPACE", scratch.path());
    std::env::set_var("OPENHUMAN_KEYRING_BACKEND", "file");
    let mut config = crate::config::Config::default();
    config.modules.enabled = true;
    config.workspace_dir = scratch.path().to_path_buf();
    config.config_path = scratch.path().join("config.toml");
    config.action_dir = scratch.path().join("action");
    config.action_dir_override = Some(config.action_dir.clone());
    config.save().await.expect("save fixture config");
    crate::security::secret_storage::init_workspace(scratch.path());

    let client = super::super::client(&config)
        .await
        .expect("pinned TinySecurity release loads");
    let policy_id = client
        .register_egress_policy(tinysecurity_bus::EgressPolicy {
            owner: tinysecurity_bus::SubjectScope {
                user: "native-fixture-user".into(),
                workspace: "native-fixture-workspace".into(),
                agent: "native-fixture-agent".into(),
            },
            ceiling: tinysecurity_bus::AccessTier::Full,
            privacy: tinysecurity_bus::PrivacyMode::Standard,
            destinations: vec![],
            data_categories: vec![tinysecurity_bus::DataCategory::OpaqueBrowserTraffic],
            local_capabilities: vec![],
            validity_ms: 30_000,
            open_public: true,
            preflight_transports: vec![tinysecurity_bus::EgressTransport::Browser],
        })
        .await
        .expect("register a host-owned browser policy");
    let generation = client.policy_info().await.expect("policy info").generation;

    for hostname in ["127.0.0.1", "169.254.169.254"] {
        let request = tinysecurity_bus::BrowserDialRequest {
            policy_id: policy_id.clone(),
            expected_generation: generation,
            session_id: "native-fixture-browser-session".into(),
            scheme: "https".into(),
            hostname: hostname.into(),
            port: 443,
        };
        let result: Result<tinysecurity_bus::BrowserDialResponse, tinybus::Error> =
            match &client.object {
                SecurityProxy::Callback(proxy) => {
                    proxy
                        .call_confidential(
                            tinysecurity_bus::names::methods::AUTHORIZE_BROWSER_DIAL,
                            (request,),
                        )
                        .await
                }
                SecurityProxy::Module(_) => panic!("production client must use its host callback"),
            };
        let response = result.unwrap_or_else(|error| panic!("module SSRF call failed: {error:?}"));
        assert!(
            response.denial.is_some(),
            "private destination {hostname} must be denied"
        );
        assert!(
            response.authorization.is_none(),
            "private destination {hostname} must receive no dial grant"
        );
    }
}
