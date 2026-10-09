use super::*;
use serde_json::json;

fn expected_cron_add() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "name": { "type": "string", "description": "Short human-readable name for the job (e.g. 'drink_water_reminder'). Always provide a name." },
            "schedule": {
                "description": "Schedule: cron expression, one-shot at time, or fixed interval.",
                "oneOf": [
                    {
                        "type": "object",
                        "description": "Repeating cron schedule. 'tz' is an IANA timezone (e.g. 'America/Los_Angeles'); defaults to device-local timezone.",
                        "properties": {
                            "kind": { "type": "string", "const": "cron" },
                            "expr": { "type": "string", "description": "Cron expression (5, 6, or 7 fields). For agent jobs, consecutive runs must be at least 5 minutes apart." },
                            "tz": { "type": "string", "description": "Optional IANA timezone name" },
                            "active_hours": {
                                "type": "object",
                                "description": "Optional: only run during these local hours",
                                "properties": {
                                    "start": { "type": "string", "description": "Start time HH:MM (e.g. '09:00')" },
                                    "end": { "type": "string", "description": "End time HH:MM (e.g. '17:00')" }
                                },
                                "required": ["start", "end"],
                                "additionalProperties": false
                            }
                        },
                        "required": ["kind", "expr"],
                        "additionalProperties": false
                    },
                    {
                        "type": "object",
                        "description": "One-shot job that runs once at a specific UTC instant.",
                        "properties": {
                            "kind": { "type": "string", "const": "at" },
                            "at": { "type": "string", "description": "ISO-8601 UTC timestamp" }
                        },
                        "required": ["kind", "at"],
                        "additionalProperties": false
                    },
                    {
                        "type": "object",
                        "description": "Repeating job that fires every N milliseconds.",
                        "properties": {
                            "kind": { "type": "string", "const": "every" },
                            "every_ms": { "type": "integer", "description": "Interval in milliseconds (must be > 0; at least 300000 = 5 minutes for agent jobs)" }
                        },
                        "required": ["kind", "every_ms"],
                        "additionalProperties": false
                    }
                ]
            },
            "job_type": { "type": "string", "enum": ["shell", "agent"] },
            "command": { "type": "string" },
            "prompt": { "type": "string" },
            "session_target": { "type": "string", "enum": ["isolated", "current", "main"], "description": "Defaults to 'current' (a fresh run that sees the recent conversation it was created in and replies there) when created inside a conversation, otherwise 'isolated'." },
            "model": { "type": "string" },
            "delivery": {
                "type": "object",
                "description": "Delivery config. Defaults to 'origin' (reply into the conversation this job was created in) when created inside a conversation, otherwise 'proactive'. Modes: origin, proactive, announce (needs channel+to), none (silent).",
                "properties": {
                    "mode": { "type": "string", "enum": ["origin", "proactive", "announce", "none"] },
                    "channel": { "type": "string", "description": "Required for announce mode" },
                    "to": { "type": "string", "description": "Required for announce mode" },
                    "best_effort": { "type": "boolean", "default": true }
                }
            },
            "delete_after_run": { "type": "boolean" }
        },
        "required": ["name", "schedule"]
    })
}

#[test]
fn cron_add_static_schema_matches_json_literal() {
    let tool = CronAddTool::new(
        std::sync::Arc::new(crate::config::Config::default()),
        std::sync::Arc::new(crate::security::SecurityPolicy::default()),
    );
    assert_eq!(
        tinytools::Tool::parameters_schema(&tool),
        expected_cron_add()
    );
}
