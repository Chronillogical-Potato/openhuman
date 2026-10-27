use super::*;

#[test]
fn tunnel_rpc_payload_requires_a_method_and_defaults_params() {
    let request = parse_tunnel_rpc_payload(serde_json::json!({"method": "devices_list"}))
        .expect("method-only payload is valid");

    assert_eq!(request.method, "devices_list");
    assert_eq!(request.params, Value::Null);
}

#[test]
fn tunnel_rpc_payload_rejects_missing_or_non_string_method() {
    for payload in [serde_json::json!({}), serde_json::json!({"method": 7})] {
        assert!(parse_tunnel_rpc_payload(payload).is_err());
    }
}
