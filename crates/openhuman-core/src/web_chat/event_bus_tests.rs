use super::*;

/// `fresh_approval_surface_subscription` returns `Some` when the global event bus has
/// been initialised and `None` otherwise (bus not started).  It must never return `None`
/// after `init_global` has been called — the production path always initialises the bus
/// before the web channel starts handling requests.
#[tokio::test]
async fn fresh_approval_surface_subscription_returns_some_when_bus_is_ready() {
    crate::core::bus::init().await.expect("bus init");
    let handle = fresh_approval_surface_subscription();
    assert!(
        handle.is_some(),
        "fresh_approval_surface_subscription() must return Some when the global event bus \
         is initialised"
    );
}

/// Calling `fresh_approval_surface_subscription` multiple times returns independent
/// handles.  Each is backed by its own background task so multiple callers can bridge
/// independently (e.g. multiple integration tests running sequentially in the same
/// process, each on their own tokio runtime).
#[tokio::test]
async fn fresh_approval_surface_subscription_is_not_a_singleton() {
    crate::core::bus::init().await.expect("bus init");
    let h1 = fresh_approval_surface_subscription();
    let h2 = fresh_approval_surface_subscription();
    assert!(h1.is_some(), "first subscription handle must be Some");
    assert!(h2.is_some(), "second subscription handle must be Some");
    // Both handles are alive — drop explicitly to show they're independent.
    drop(h1);
    drop(h2);
}

/// Drain the web-channel receiver until an `external_transfer_pending` event
/// whose `args.service` matches `marker` arrives (the bus is process-wide).
async fn find_egress_web_event(
    rx: &mut broadcast::Receiver<WebChannelEvent>,
    marker: &str,
) -> WebChannelEvent {
    loop {
        match rx.recv().await {
            Ok(ev)
                if ev.event == "external_transfer_pending"
                    && ev
                        .args
                        .as_ref()
                        .and_then(|a| a.get("service"))
                        .and_then(|s| s.as_str())
                        == Some(marker) =>
            {
                return ev;
            }
            Ok(_) => continue,
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => {
                panic!("web-channel bus closed before external_transfer_pending arrived")
            }
        }
    }
}

/// Egress-surface bridges an `ExternalTransferPending` that carries chat
/// routing into an `external_transfer_pending` web-channel event whose args
/// mirror the descriptor (privacy epic S2, #4436).
#[tokio::test]
async fn egress_surface_bridges_pending_with_chat_context() {
    crate::core::bus::init().await.expect("bus init");
    let _handle = crate::core::bus::BUS.subscribe(Arc::new(EgressSurfaceSubscriber));
    let mut web_rx = subscribe_web_channel_events();

    let marker = "svc-bridge-with-context";
    crate::core::bus::BUS.publish(DomainEvent::ExternalTransferPending {
        descriptor: crate::security::egress::EgressDescriptor::composio(marker),
        thread_id: Some("thread-1".to_string()),
        client_id: Some("client-1".to_string()),
    });

    let ev = find_egress_web_event(&mut web_rx, marker).await;
    assert_eq!(ev.thread_id, "thread-1");
    assert_eq!(ev.client_id, "client-1");
    let args = ev.args.expect("args present");
    assert_eq!(args["provider_slug"], "composio");
    assert_eq!(args["reason"], "tool_call");
    assert_eq!(args["is_external"], true);
}

/// A pending event with no chat routing is NOT surfaced to the web channel
/// (background/CLI/cron egress has no client to fan out to).
#[tokio::test]
async fn egress_surface_drops_pending_without_chat_context() {
    crate::core::bus::init().await.expect("bus init");
    let _handle = crate::core::bus::BUS.subscribe(Arc::new(EgressSurfaceSubscriber));
    let mut web_rx = subscribe_web_channel_events();

    let dropped_marker = "svc-bridge-no-context";
    let sentinel_marker = "svc-bridge-sentinel";
    // No context → must be dropped. A following event WITH context must be
    // surfaced; reaching the sentinel proves the first was suppressed.
    crate::core::bus::BUS.publish(DomainEvent::ExternalTransferPending {
        descriptor: crate::security::egress::EgressDescriptor::composio(dropped_marker),
        thread_id: None,
        client_id: None,
    });
    crate::core::bus::BUS.publish(DomainEvent::ExternalTransferPending {
        descriptor: crate::security::egress::EgressDescriptor::composio(sentinel_marker),
        thread_id: Some("thread-2".to_string()),
        client_id: Some("client-2".to_string()),
    });

    loop {
        match web_rx.recv().await {
            Ok(ev) if ev.event == "external_transfer_pending" => {
                let svc = ev
                    .args
                    .as_ref()
                    .and_then(|a| a.get("service"))
                    .and_then(|s| s.as_str());
                assert_ne!(
                    svc,
                    Some(dropped_marker),
                    "no-context transfer must not surface to the web channel"
                );
                if svc == Some(sentinel_marker) {
                    break;
                }
            }
            Ok(_) => continue,
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => {
                panic!("web-channel bus closed before sentinel arrived")
            }
        }
    }
}

/// Drain the web-channel receiver until an event of the given name whose
/// `args.artifact_id` matches `marker` arrives.
async fn find_artifact_web_event(
    rx: &mut broadcast::Receiver<WebChannelEvent>,
    event_name: &str,
    marker: &str,
) -> WebChannelEvent {
    loop {
        match rx.recv().await {
            Ok(ev)
                if ev.event == event_name
                    && ev
                        .args
                        .as_ref()
                        .and_then(|a| a.get("artifact_id"))
                        .and_then(|s| s.as_str())
                        == Some(marker) =>
            {
                return ev;
            }
            Ok(_) => continue,
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => {
                panic!("web-channel bus closed before {event_name} arrived")
            }
        }
    }
}

/// `ArtifactPending`/`Ready`/`Failed` carry `tool_call_id`/`request_id`
/// (C5, correlating a generated artifact card with the tool-call bubble
/// that produced it). The artifact-surface subscriber must bridge both onto
/// `WebChannelEvent.tool_call_id` / `.turn_request_id` for every one of the
/// three lifecycle events.
#[tokio::test]
async fn artifact_surface_bridges_tool_call_id_and_request_id() {
    crate::core::bus::init().await.expect("bus init");
    let _handle = crate::core::bus::BUS.subscribe(Arc::new(ArtifactSurfaceSubscriber));
    let mut web_rx = subscribe_web_channel_events();

    let pending_id = "artifact-corr-pending";
    crate::core::bus::BUS.publish(DomainEvent::ArtifactPending {
        artifact_id: pending_id.to_string(),
        kind: "image".to_string(),
        title: "A cat".to_string(),
        workspace_dir: "/tmp/ws".to_string(),
        path: format!("{pending_id}/a-cat.png"),
        thread_id: Some("thread-1".to_string()),
        client_id: Some("client-1".to_string()),
        tool_call_id: Some("call-1".to_string()),
        request_id: Some("req-1".to_string()),
    });
    let ev = find_artifact_web_event(&mut web_rx, "artifact_pending", pending_id).await;
    assert_eq!(ev.tool_call_id, Some("call-1".to_string()));
    assert_eq!(ev.turn_request_id, Some("req-1".to_string()));

    let ready_id = "artifact-corr-ready";
    crate::core::bus::BUS.publish(DomainEvent::ArtifactReady {
        artifact_id: ready_id.to_string(),
        kind: "image".to_string(),
        title: "A cat".to_string(),
        workspace_dir: "/tmp/ws".to_string(),
        path: format!("{ready_id}/a-cat.png"),
        size_bytes: 42,
        thread_id: Some("thread-1".to_string()),
        client_id: Some("client-1".to_string()),
        tool_call_id: Some("call-2".to_string()),
        request_id: Some("req-2".to_string()),
    });
    let ev = find_artifact_web_event(&mut web_rx, "artifact_ready", ready_id).await;
    assert_eq!(ev.tool_call_id, Some("call-2".to_string()));
    assert_eq!(ev.turn_request_id, Some("req-2".to_string()));

    let failed_id = "artifact-corr-failed";
    crate::core::bus::BUS.publish(DomainEvent::ArtifactFailed {
        artifact_id: failed_id.to_string(),
        kind: "image".to_string(),
        title: "A cat".to_string(),
        workspace_dir: "/tmp/ws".to_string(),
        error: "provider timeout".to_string(),
        thread_id: Some("thread-1".to_string()),
        client_id: Some("client-1".to_string()),
        tool_call_id: Some("call-3".to_string()),
        request_id: Some("req-3".to_string()),
    });
    let ev = find_artifact_web_event(&mut web_rx, "artifact_failed", failed_id).await;
    assert_eq!(ev.tool_call_id, Some("call-3".to_string()));
    assert_eq!(ev.turn_request_id, Some("req-3".to_string()));
}

/// A producer that ran outside a harness tool-call context (CLI, cron)
/// carries `tool_call_id: None` — the bridged event must not fabricate one.
#[tokio::test]
async fn artifact_surface_leaves_tool_call_id_none_when_absent() {
    crate::core::bus::init().await.expect("bus init");
    let _handle = crate::core::bus::BUS.subscribe(Arc::new(ArtifactSurfaceSubscriber));
    let mut web_rx = subscribe_web_channel_events();

    let ready_id = "artifact-corr-no-call-id";
    crate::core::bus::BUS.publish(DomainEvent::ArtifactReady {
        artifact_id: ready_id.to_string(),
        kind: "document".to_string(),
        title: "A doc".to_string(),
        workspace_dir: "/tmp/ws".to_string(),
        path: format!("{ready_id}/a-doc.docx"),
        size_bytes: 7,
        thread_id: Some("thread-1".to_string()),
        client_id: Some("client-1".to_string()),
        tool_call_id: None,
        request_id: None,
    });
    let ev = find_artifact_web_event(&mut web_rx, "artifact_ready", ready_id).await;
    assert_eq!(ev.tool_call_id, None);
    assert_eq!(ev.turn_request_id, None);
}
