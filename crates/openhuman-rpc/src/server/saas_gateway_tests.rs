use super::*;

#[test]
fn single_user_surfaces_are_closed() {
    for path in [
        "/v1",
        "/v1/chat/completions",
        "/events",
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
    for path in ["/", "/health", "/schema", "/rpc", "/v1x", "/eventsource"] {
        assert!(!is_closed_in_saas(path), "{path}");
    }
}
