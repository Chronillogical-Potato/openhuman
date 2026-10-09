use super::*;
use serde_json::json;

fn expected_browser() -> serde_json::Value {
    json!({"type":"object","properties":{
        "action":{"type":"string","enum":["open","snapshot","read_page","click","fill","type","get_text","get_title","get_url","wait","press","hover","scroll","is_visible","find","task","task_continue","task_cancel","confirm_pending","list_downloads","wait_download","close"]},
        "url":{"type":"string","description":"Starting HTTPS URL for open or an optional starting URL for task"},"selector":{"type":"string"},"value":{"type":"string"},"text":{"type":"string"},"key":{"type":"string"},"direction":{"type":"string"},"pixels":{"type":"integer"},"ms":{"type":"integer"},"timeout_ms":{"type":"integer"},"interactive_only":{"type":"boolean"},"compact":{"type":"boolean"},"depth":{"type":"integer"},"by":{"type":"string"},"find_action":{"type":"string"},"fill_value":{"type":"string"},"goal":{"type":"string"},"inputs":{"type":"object","additionalProperties":{"type":"string"}},"task_id":{"type":"string","description":"Task id returned by task, for task_continue and task_cancel"},"flow":{"type":"object","description":"Optional TinyComputer flow ({app, vars, steps}) to run instead of planning one from goal, e.g. a plan saved from an earlier successful run"},"answer":{"type":"string","description":"Free-text answer for a paused task; done after a needs_human pause"},"token":{"type":"string","description":"Token returned with the exact pending action"}
    },"required":["action"]})
}

#[test]
fn browser_static_schema_matches_json_literal() {
    let tool = BrowserTool::new(
        std::sync::Arc::new(crate::security::SecurityPolicy::default()),
        std::sync::Arc::new(BrowserClient::new(std::sync::Arc::new(
            crate::config::Config::default(),
        ))),
        3,
    );
    assert_eq!(
        tinytools::Tool::parameters_schema(&tool),
        expected_browser()
    );
}
