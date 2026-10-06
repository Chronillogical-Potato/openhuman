use super::*;

fn mk(namespace: &'static str, function: &'static str) -> ControllerSchema {
    ControllerSchema {
        namespace,
        function,
        description: "",
        inputs: vec![],
        outputs: vec![],
    }
}

#[test]
fn method_name_joins_namespace_and_function_with_dot() {
    let s = mk("memory", "doc_put");
    assert_eq!(s.method_name(), "memory.doc_put");
}

#[test]
fn method_name_is_not_an_rpc_method_name() {
    // The dotted controller key and the `openhuman.<ns>_<fn>` RPC method
    // name are intentionally different — guard against drift.
    let s = mk("memory", "doc_put");
    assert_eq!(s.method_name(), "memory.doc_put");
    assert_eq!(
        crate::core::all::rpc_method_name(&s),
        "openhuman.memory_doc_put"
    );
}

#[test]
fn method_name_preserves_underscores_in_function() {
    let s = mk("team", "change_member_role");
    assert_eq!(s.method_name(), "team.change_member_role");
}

#[test]
fn controller_schema_serializes_to_json() {
    // Schema must be JSON-serializable: the /schema endpoint depends on it.
    let s = ControllerSchema {
        namespace: "health",
        function: "snapshot",
        description: "d",
        inputs: vec![FieldSchema {
            name: "limit",
            ty: TypeSchema::U64,
            comment: "cap",
            required: false,
        }],
        outputs: vec![FieldSchema {
            name: "ok",
            ty: TypeSchema::Bool,
            comment: "",
            required: true,
        }],
    };
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["namespace"], "health");
    assert_eq!(json["function"], "snapshot");
    assert_eq!(json["inputs"][0]["name"], "limit");
    assert_eq!(json["outputs"][0]["required"], true);
}

#[test]
fn bounded_u64_serializes_additively() {
    // `/schema` is a public contract: the bound rides in a new variant, so
    // existing `"U64"` fields keep their exact wire shape (#6137).
    let bounded = TypeSchema::BoundedU64 {
        min: 1,
        max: u32::MAX as u64,
    };
    assert_eq!(
        serde_json::to_value(&bounded).unwrap(),
        serde_json::json!({ "BoundedU64": { "min": 1, "max": 4_294_967_295u64 } })
    );
    assert_eq!(
        serde_json::to_value(TypeSchema::U64).unwrap(),
        serde_json::json!("U64")
    );
}
