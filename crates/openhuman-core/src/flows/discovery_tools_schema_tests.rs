use super::*;
use serde_json::json;

fn expected_suggest_workflows() -> serde_json::Value {
    json!({
            "type": "object",
            "properties": {
                "suggestions": {
                    "type": "array",
                    "description": "1-8 workflow suggestions, highest-value first.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "title": {
                                "type": "string",
                                "description": "Short, human-friendly title, e.g. \"Auto-file email receipts\"."
                            },
                            "one_liner": {
                                "type": "string",
                                "description": "One sentence describing what the workflow does."
                            },
                            "rationale": {
                                "type": "string",
                                "description": "Why this is suggested to THIS user, grounded in what you observed."
                            },
                            "trigger_hint": {
                                "type": "string",
                                "description": "Likely trigger: \"schedule\" | \"app_event\" | \"manual\" (only these self-fire).",
                                "enum": ["schedule", "app_event", "manual"]
                            },
                            "steps_outline": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Plain-language step outline, one per element."
                            },
                            "suggested_connections": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Real connection_ref values from list_flow_connections. Never invented."
                            },
                            "suggested_slugs": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Real Composio action slugs from search_tool_catalog. Never hallucinated."
                            },
                            "build_prompt": {
                                "type": "string",
                                "description": "Self-contained natural-language brief handed to the workflow-builder on \"Build this\"."
                            },
                            "confidence": {
                                "type": "number",
                                "description": "Your confidence in [0,1] that this is a genuinely useful, buildable automation."
                            }
                        },
                        "required": ["title", "one_liner", "rationale", "build_prompt"]
                    }
                },
                "run_id": {
                    "type": "string",
                    "description": "Optional correlation id for the discovery run that produced these."
                }
            },
            "required": ["suggestions"]
        })
}

#[test]
fn suggest_workflows_static_schema_matches_json_literal() {
    let tool = todo!("construct tool");
    assert_eq!(Tool::parameters_schema(&tool), expected_suggest_workflows());
}
