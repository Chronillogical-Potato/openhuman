use super::*;
use serde_json::json;

fn expected_save_workflow() -> serde_json::Value {
    json!({
            "type": "object",
            "properties": {
                "flow_id": {
                    "type": "string",
                    "description": "Id of the EXISTING saved flow to write the graph to (the persistence target — always required)."
                },
                "draft_id": {
                    "type": "string",
                    "description": "A working draft whose graph to persist onto the flow. Provide this OR inline `graph`; if both are given, draft_id wins."
                },
                "graph": {
                    "type": "object",
                    "description": "The full tinyflows WorkflowGraph to persist: { name?, nodes: [...], edges: [...] }. Provide this OR `draft_id`. Same shape as propose_workflow.",
                    "properties": {
                        "nodes": { "type": "array" },
                        "edges": { "type": "array" }
                    },
                    "required": ["nodes", "edges"]
                },
                "name": {
                    "type": "string",
                    "description": "Optional new human-readable name for the flow."
                }
            },
            "required": ["flow_id"],
            "additionalProperties": false
        })
}

#[test]
fn save_workflow_static_schema_matches_json_literal() {
    let tool = todo!("construct tool");
    assert_eq!(Tool::parameters_schema(&tool), expected_save_workflow());
}
