use super::*;
use serde_json::json;

fn expected_propose_workflow() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "name": {
                "type": "string",
                "description": "Human-readable name for the proposed flow."
            },
            "graph": {
                "type": "object",
                "description": "A tinyflows WorkflowGraph: { name?, nodes: [...], edges: [...] }. See the tool description for node kinds and their config shapes.",
                "properties": {
                    "nodes": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string", "description": "Unique id within the graph." },
                                "kind": {
                                    "type": "string",
                                    "enum": [
                                        "trigger", "agent", "tool_call", "http_request",
                                        "code", "shell", "condition", "switch", "merge", "split_out",
                                        "transform", "output_parser", "sub_workflow", "memory",
                                        "dedup", "loop", "spawn", "gate", "scatter", "gather",
                                        "approval", "void"
                                    ]
                                },
                                "name": { "type": "string", "description": "Human-readable node name." },
                                "config": { "description": "Kind-specific configuration; see tool description." }
                            },
                            "required": ["id", "kind", "name"]
                        }
                    },
                    "edges": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "from_node": { "type": "string" },
                                "to_node": { "type": "string" },
                                "from_port": { "type": "string", "description": "Defaults to \"main\". For a condition/switch branch, this is where the branch label (e.g. \"true\"/\"false\") goes." },
                                "to_port": { "type": "string", "description": "Defaults to \"main\". Almost always stays \"main\" — branch labels go on from_port, not here." }
                            },
                            "required": ["from_node", "to_node"]
                        }
                    }
                },
                "required": ["nodes", "edges"]
            },
            "require_approval": {
                "type": "boolean",
                "description": "Force a human-approval gate on every outbound tool/HTTP action this flow takes once saved. Defaults to true for agent-proposed flows."
            }
        },
        "required": ["name", "graph"]
    })
}

#[test]
fn propose_workflow_static_schema_matches_json_literal() {
    let tool = ProposeWorkflowTool::new(std::sync::Arc::new(crate::config::Config::default()));
    assert_eq!(
        tinytools::Tool::parameters_schema(&tool),
        expected_propose_workflow()
    );
}
