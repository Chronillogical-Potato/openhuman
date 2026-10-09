use super::*;
use serde_json::json;

fn expected_edit_workflow() -> serde_json::Value {
    json!({
            "type": "object",
            "properties": {
                "draft_id": {
                    "type": "string",
                    "description": "A working draft to edit as the base; the applied edit is written back to it. Provide one of draft_id / flow_id / graph."
                },
                "flow_id": {
                    "type": "string",
                    "description": "The saved flow to edit as the base graph. Provide one of draft_id / flow_id / graph."
                },
                "graph": {
                    "type": "object",
                    "description": "An inline base tinyflows WorkflowGraph to edit. Provide one of draft_id / flow_id / graph.",
                    "properties": {
                        "nodes": { "type": "array" },
                        "edges": { "type": "array" }
                    }
                },
                "ops": {
                    "type": "array",
                    "description": "The structured edits, applied in order. Each item is { op, ... } — see the tool description for op shapes.",
                    "items": { "type": "object", "properties": { "op": { "type": "string" } }, "required": ["op"] },
                    "minItems": 1
                },
                "name": {
                    "type": "string",
                    "description": "Name for the resulting proposed flow. Defaults to the base flow's name."
                },
                "instruction": {
                    "type": "string",
                    "description": "The change that motivated these ops (echoed back on the review card)."
                },
                "require_approval": {
                    "type": "boolean",
                    "description": "Force a human-approval gate on every outbound action once saved. Defaults to true."
                }
            },
            "required": ["ops"]
        })
}

#[test]
fn edit_workflow_static_schema_matches_json_literal() {
    let tool = todo!("construct tool");
    assert_eq!(Tool::parameters_schema(&tool), expected_edit_workflow());
}
