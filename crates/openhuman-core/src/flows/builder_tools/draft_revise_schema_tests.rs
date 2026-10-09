use super::*;
use serde_json::json;

fn expected_revise_workflow() -> serde_json::Value {
    json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Human-readable name for the (revised) proposed flow."
                },
                "graph": {
                    "type": "object",
                    "description": "The full REVISED tinyflows WorkflowGraph: { name?, nodes: [...], edges: [...] }. Apply your changes to the prior draft and pass the whole graph — see propose_workflow for node kinds and config shapes.",
                    "properties": {
                        "nodes": { "type": "array" },
                        "edges": { "type": "array" }
                    },
                    "required": ["nodes", "edges"]
                },
                "instruction": {
                    "type": "string",
                    "description": "The revision instruction that motivated this change (e.g. 'add a Slack step after the summary'). Echoed back for the review card; does not affect validation."
                },
                "require_approval": {
                    "type": "boolean",
                    "description": "Force a human-approval gate on every outbound action once saved. Defaults to true for agent-proposed flows."
                }
            },
            "required": ["name", "graph"]
        })
}

#[test]
fn revise_workflow_static_schema_matches_json_literal() {
    let tool = todo!("construct tool");
    assert_eq!(Tool::parameters_schema(&tool), expected_revise_workflow());
}
