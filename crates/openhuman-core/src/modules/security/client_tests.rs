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
async fn device_pairing_and_frame_calls_use_the_confidential_security_bus() {
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
    if let SecurityProxy::Module(object) = &client.object {
        object
            .call_detailed::<tinysecurity_bus::DevicePairingResponse2>(
                tinysecurity_bus::names::methods::DEVICE_PAIRING2,
                (pairing.clone(),),
            )
            .await
            .unwrap_or_else(|error| panic!("detailed fixture call failed: {error:?}"));
    }
    let pairing_result = client.device_pairing2(pairing.clone()).await.unwrap();
    assert_eq!(pairing_result.frame, [3]);
    assert_eq!(*fixture.pairing.lock().unwrap(), Some(pairing));

    let frame = tinysecurity_bus::DeviceFrameRequest2 {
        scope_id: "scope".into(),
        handle: "opaque-session".into(),
        call_id: "frame-call".into(),
        operation: tinysecurity_bus::DeviceFrameOperation2::Open { frame: vec![7] },
    };
    let frame_result = client.device_frame2(frame.clone()).await.unwrap();
    assert_eq!(frame_result.bytes, [2]);
    assert_eq!(*fixture.frame.lock().unwrap(), Some(frame));
}
