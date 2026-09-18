use super::*;

#[test]
fn catalog_counts_match_and_nonempty() {
    let s = all_controller_schemas();
    let h = all_registered_controllers();
    assert_eq!(s.len(), h.len());
    assert!(
        s.len() >= 10,
        "local inference should expose >=10 controller fns"
    );
}

#[test]
fn all_schemas_use_inference_namespace_and_have_descriptions() {
    for s in all_controller_schemas() {
        assert_eq!(s.namespace, "inference", "function {}", s.function);
        assert!(!s.description.is_empty(), "function {} desc", s.function);
        assert!(!s.outputs.is_empty(), "function {} outputs", s.function);
    }
}

#[test]
fn unknown_function_returns_unknown_schema() {
    let s = schemas("no_such_fn");
    assert_eq!(s.function, "unknown");
    assert_eq!(s.namespace, "inference");
}

#[test]
fn every_registered_key_resolves_to_non_unknown_schema() {
    let keys = [
        "agent_chat",
        "agent_chat_simple",
        "transcribe",
        "transcribe_bytes",
        "tts",
        "assets_status",
        "downloads_progress",
        "download_asset",
        "install_piper",
        "piper_install_status",
        "test_connection",
    ];
    for k in keys {
        let s = schemas(k);
        assert_eq!(s.namespace, "inference");
        assert_ne!(s.function, "unknown", "key `{k}` fell through");
    }
}

#[test]
fn registered_controllers_all_in_inference_namespace() {
    for h in all_registered_controllers() {
        assert_eq!(h.schema.namespace, "inference");
        assert!(!h.schema.function.is_empty());
    }
}

#[test]
fn field_builder_helpers_are_correct_shape() {
    let r = required_string("k", "c");
    assert!(r.required);
    assert!(matches!(r.ty, TypeSchema::String));

    let o = optional_string("k", "c");
    assert!(!o.required);

    let ou = optional_u64("k", "c");
    assert!(!ou.required);

    let j = json_output("result", "c");
    assert!(j.required);
    assert!(matches!(j.ty, TypeSchema::Json));
}

#[test]
fn to_json_wraps_rpc_outcome() {
    let v =
        to_json(RpcOutcome::single_log(serde_json::json!({"ok": true}), "l")).expect("serialize");
    assert!(v.get("logs").is_some() || v.get("result").is_some() || v.get("ok").is_some());
}

#[test]
fn deserialize_params_parses_valid_object() {
    let mut m = Map::new();
    m.insert("message".into(), Value::String("hi".into()));
    let p: AgentChatParams = deserialize_params(m).expect("parse");
    assert_eq!(p.message, "hi");
}

#[test]
fn deserialize_params_errors_on_invalid_shape() {
    let mut m = Map::new();
    m.insert("message".into(), Value::Bool(true));
    let err = deserialize_params::<AgentChatParams>(m).unwrap_err();
    assert!(err.contains("invalid params"));
}

/// `agent_id` is a declared, optional input of `inference.agent_chat` — the
/// embed crate pins its `TurnRequest` field names against this schema, so a
/// param the controller accepts but never declares would be unreachable.
#[test]
fn agent_chat_declares_an_optional_agent_id() {
    let schema = schemas("agent_chat");
    let field = schema
        .inputs
        .iter()
        .find(|f| f.name == "agent_id")
        .expect("agent_chat declares agent_id");
    assert!(!field.required, "agent_id must stay optional");
}

/// The params struct accepts the field and, absent, defaults it to `None`, so
/// every existing caller keeps running the orchestrator.
#[test]
fn agent_chat_params_default_agent_id_to_none() {
    let without: AgentChatParams =
        serde_json::from_value(serde_json::json!({ "message": "hi" })).expect("decodes");
    assert!(without.agent_id.is_none());
    let with: AgentChatParams =
        serde_json::from_value(serde_json::json!({ "message": "hi", "agent_id": "alpha" }))
            .expect("decodes");
    assert_eq!(with.agent_id.as_deref(), Some("alpha"));
}
