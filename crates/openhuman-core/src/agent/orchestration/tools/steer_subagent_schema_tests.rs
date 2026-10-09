use super::*;
use serde_json::json;

fn expected_steer_subagent() -> serde_json::Value {
    json!({
            "type": "object",
            "required": ["message"],
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "Transient task_id returned by reusable async delegation."
                },
                "subagent_session_id": {
                    "type": "string",
                    "description": "Durable subagent_session_id returned by reusable async delegation. Preferred over task_id for cross-turn messaging."
                },
                "message": {
                    "type": "string",
                    "description": "Instruction or data to inject into the running sub-agent."
                },
                "mode": {
                    "type": "string",
                    "enum": ["steer", "collect"],
                    "default": "steer",
                    "description": "steer = a new instruction the sub-agent must address; collect = silent additional context."
                }
            }
        })
}

#[test]
fn steer_subagent_static_schema_matches_json_literal() {
    let tool = todo!("construct tool");
    assert_eq!(Tool::parameters_schema(&tool), expected_steer_subagent());
}
