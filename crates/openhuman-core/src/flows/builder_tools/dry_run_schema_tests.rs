use super::*;
use serde_json::json;

fn expected_dry_run_workflow() -> serde_json::Value {
    json!({
            "type": "object",
            "properties": {
                "draft_id": {
                    "type": "string",
                    "description": "A working draft to simulate. Provide one of draft_id / flow_id / graph (draft_id wins)."
                },
                "flow_id": {
                    "type": "string",
                    "description": "A saved flow to simulate. Provide one of draft_id / flow_id / graph."
                },
                "graph": {
                    "type": "object",
                    "description": "An inline tinyflows WorkflowGraph to simulate: { nodes: [...], edges: [...] }. Provide one of draft_id / flow_id / graph.",
                    "properties": {
                        "nodes": { "type": "array" },
                        "edges": { "type": "array" }
                    },
                    "required": ["nodes", "edges"]
                },
                "input": {
                    "description": "Optional trigger input passed to the run (defaults to {})."
                }
            }
        })
}

#[test]
fn dry_run_workflow_static_schema_matches_json_literal() {
    let tool = DryRunWorkflowTool::new(std::sync::Arc::new(crate::config::Config::default()));
    assert_eq!(tinytools::Tool::parameters_schema(&tool), expected_dry_run_workflow());
}
