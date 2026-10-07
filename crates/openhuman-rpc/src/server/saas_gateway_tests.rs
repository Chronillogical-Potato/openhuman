use super::*;

#[test]
fn single_user_surfaces_are_closed() {
    for path in [
        "/v1",
        "/v1/chat/completions",
        "/events/domain",
        "/events/webhooks",
        "/ws/dictation",
        "/socket.io/",
        "/dev/connect",
        "/oauth/mcp/callback",
    ] {
        assert!(is_closed_in_saas(path), "{path}");
    }
}

#[test]
fn the_gateway_surfaces_stay_open() {
    // `/events` itself is the per-user chat stream, gated on the user scope by
    // the layer rather than closed by prefix.
    for path in ["/", "/health", "/schema", "/rpc", "/v1x", "/eventsource", "/events"] {
        assert!(!is_closed_in_saas(path), "{path}");
    }
}
