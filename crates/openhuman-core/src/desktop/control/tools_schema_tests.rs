use super::*;
use serde_json::json;

fn expected_schemas() -> Vec<(DesktopToolKind, serde_json::Value)> {
    vec![
        (
            DesktopToolKind::Apps,
            json!({"type":"object","properties":{}}),
        ),
        (
            DesktopToolKind::Windows,
            json!({"type":"object","properties":{
                "app":{"type":"string","description":"Optional application name"}}}),
        ),
        (
            DesktopToolKind::Launch,
            json!({"type":"object","properties":{
                "app":{"type":"string","description":"Application name, e.g. Spotify or TextEdit"}},
                "required":["app"],"additionalProperties":false}),
        ),
        (
            DesktopToolKind::Snapshot,
            json!({"type":"object","properties":{
                "app":{"type":"string"}, "window_id":{"type":"string","minLength":1,"description":"Exact window ID from desktop_list_windows"}, "skeleton":{"type":"boolean"},
                "root_ref":{"type":"string"}, "max_depth":{"type":"integer","minimum":1,"maximum":12}}}),
        ),
        (
            DesktopToolKind::Find,
            json!({"type":"object","properties":{
                "app":{"type":"string"},"role":{"type":"string"},"name":{"type":"string"},
                "root":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":20}}}),
        ),
        (
            DesktopToolKind::Act,
            json!({"type":"object","properties":{
                "operation":{"type":"string","enum":["click","focus","type","check","uncheck","expand","collapse"]},
                "ref_id":{"type":"string","description":"Snapshot-qualified ref from desktop_snapshot or desktop_find"},
                "text":{"type":"string","description":"Required for type"}},
                "required":["operation","ref_id"]}),
        ),
        (
            DesktopToolKind::Goal,
            json!({"type":"object","properties":{
                "app":{"type":"string","description":"Native app to control"},
                "window":{"type":"string","description":"Optional exact title of an observed app window"},
                "window_id":{"type":"string","minLength":1,"description":"Optional exact window ID from desktop_list_windows; binds actions and verification to that native window"},
                "goal":{"type":"string","description":"One bounded desktop task; describe the intended visible result"},
                "allowed_operations":{"type":"array","minItems":1,"items":{"type":"string","enum":["CLICK","TYPE_TEXT","CHECK","UNCHECK","EXPAND","COLLAPSE","SCROLL"]},"description":"Mutating operations Jev may execute; enumerate those needed for this task"},
                "allowed_targets":{"type":"array","minItems":1,"items":{"type":"string"},"description":"Exact accessible name, description, or native_id.value from desktop_snapshot or desktop_find for each action target"},
                "text_slots":{"type":"object","additionalProperties":{"type":"string"},"description":"Prepared text keyed by an exact allowed target. Copy user supplied text verbatim; do not normalize punctuation, spacing, or case"},
                "success":{"type":"array","minItems":1,"items":{"type":"object","properties":{
                    "kind":{"type":"string","enum":["name_present","name_contains","value_equals","value_contains","state_contains"]},
                    "name":{"type":"string","description":"Exact accessible name, description, or native_id.value from the snapshot for name_present, value_equals, value_contains, or state_contains"},
                    "fragment":{"type":"string","description":"For name_contains, a stable substring of the visible descendant name, such as the exact outgoing message text"},
                    "within":{"type":"string","description":"For name_contains, the exact accessible name of the ancestor container, such as the active conversation message list"},
                    "value":{"type":"string"},"state":{"type":"string"}},
                    "required":["kind"]},"description":"All predicates must match a fresh accessibility observation before completion. name_contains requires fragment and within"},
                "max_steps":{"type":"integer","minimum":1,"maximum":20},
                "max_model_calls":{"type":"integer","minimum":1,"maximum":40},
                "max_elapsed_ms":{"type":"integer","minimum":1000,"maximum":300000}},
                "required":["app","goal","allowed_operations","allowed_targets","success"],"additionalProperties":false}),
        ),
        (
            DesktopToolKind::ContinueGoal,
            json!({"type":"object","properties":{
                "confirmation_id":{"type":"string","description":"One-use handle approved by the user in Connections"}},
                "required":["confirmation_id"]}),
        ),
    ]
}

#[test]
fn desktop_static_schemas_match_json_literals() {
    let config = std::sync::Arc::new(crate::config::Config::default());
    for (kind, expected) in expected_schemas() {
        let tool = DesktopTool::new(config.clone(), kind);
        assert_eq!(
            tinytools::Tool::parameters_schema(&tool),
            expected,
            "{}",
            tinytools::Tool::name(&tool)
        );
    }
}
