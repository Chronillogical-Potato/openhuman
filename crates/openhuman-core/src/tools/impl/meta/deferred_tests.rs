use std::collections::HashSet;

use serde_json::json;
use tinytools::{Tool, ToolExposure, ToolResult};

use super::{deferred_tool_names, strip_deferred_from_visible};

struct Fake(&'static str, ToolExposure);

#[async_trait::async_trait]
impl Tool for Fake {
    fn name(&self) -> &str {
        self.0
    }
    fn description(&self) -> &str {
        "fake"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({"type": "object"})
    }
    fn exposure(&self) -> ToolExposure {
        self.1
    }
    async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<ToolResult> {
        Ok(ToolResult::success("ok"))
    }
}

fn tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(Fake("direct", ToolExposure::Direct)),
        Box::new(Fake("deferred", ToolExposure::Deferred)),
        Box::new(Fake("hidden", ToolExposure::Hidden)),
        Box::new(Fake("unlisted_deferred", ToolExposure::Deferred)),
    ]
}

#[test]
fn strip_removes_deferred_and_hidden_but_returns_only_deferred() {
    let mut visible: HashSet<String> = ["direct", "deferred", "hidden"]
        .into_iter()
        .map(String::from)
        .collect();
    let deferred = strip_deferred_from_visible(&mut visible, &tools());
    assert_eq!(visible, HashSet::from(["direct".to_string()]));
    assert_eq!(deferred, HashSet::from(["deferred".to_string()]));
}

#[test]
fn strip_ignores_tools_the_belt_never_named() {
    let mut visible: HashSet<String> = HashSet::from(["direct".to_string()]);
    let deferred = strip_deferred_from_visible(&mut visible, &tools());
    assert!(deferred.is_empty());
    assert_eq!(visible.len(), 1);
}

#[test]
fn deferred_tool_names_lists_every_deferred_registration() {
    assert_eq!(
        deferred_tool_names(&tools()),
        HashSet::from(["deferred".to_string(), "unlisted_deferred".to_string()])
    );
}
