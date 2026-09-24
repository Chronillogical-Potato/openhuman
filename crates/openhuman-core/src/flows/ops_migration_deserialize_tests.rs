use super::*;

/// The member scan covers four arrays, and until now only `nodes` was ever
/// exercised — the other three arms were unreachable in the test suite while
/// looking correct by inspection.
#[test]
fn migrate_and_deserialize_graph_names_the_member_in_inputs_agents_and_edges() {
    let bad_input = json!({ "inputs": [ { "type": "string" } ] });
    let err = migrate_and_deserialize_graph(bad_input).expect_err("input without `name`");
    assert!(err.starts_with("inputs[0]: "), "got: {err}");

    let bad_agent = json!({ "agents": [ { "name": "no id" } ] });
    let err = migrate_and_deserialize_graph(bad_agent).expect_err("agent without `id`");
    assert!(err.starts_with("agents[0]: "), "got: {err}");

    let bad_edge = json!({ "edges": [ { "from_port": "main" } ] });
    let err = migrate_and_deserialize_graph(bad_edge).expect_err("edge without endpoints");
    assert!(err.starts_with("edges[0]: "), "got: {err}");
}

#[test]
fn migrate_and_deserialize_graph_falls_back_when_no_member_is_at_fault() {
    let nodes_not_an_array = json!({ "name": "valid", "nodes": "this is not an array" });
    let err = migrate_and_deserialize_graph(nodes_not_an_array)
        .expect_err("a non-array `nodes` must not deserialize");
    assert!(!err.contains("nodes["), "got: {err}");
    assert!(err.contains("invalid type"), "got: {err}");
}

#[test]
fn migrate_and_deserialize_graph_reports_a_non_object_graph_without_inventing_a_location() {
    let err = migrate_and_deserialize_graph(json!("this is a string, not a workflow graph"))
        .expect_err("a non-object graph must not deserialize");
    assert!(!err.contains('['), "got: {err}");
    assert!(
        !err.contains(": invalid") || !err.starts_with("name"),
        "got: {err}"
    );
    assert!(err.contains("invalid type"), "got: {err}");
}
