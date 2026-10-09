use super::*;
use serde_json::json;

fn expected_node_exec() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "inline_code": {
                "type": "string",
                "description": "JavaScript source passed to `node -e`. Mutually exclusive with script_path."
            },
            "script_path": {
                "type": "string",
                "description": "Path (relative to workspace) to a .js/.mjs/.cjs file. Mutually exclusive with inline_code."
            },
            "args": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Positional arguments appended after the script. Ignored for inline_code."
            },
            "timeout_secs": {
                "type": "integer",
                "description": "Optional wall-clock timeout (seconds) before the process is killed. No timeout by default — long-running scripts run to completion. Capped at 1800s; 0 disables."
            }
        }
    })
}

#[test]
fn node_exec_static_schema_matches_json_literal() {
    let tool = NodeExecTool::new(
        std::sync::Arc::new(crate::security::SecurityPolicy::default()),
        std::sync::Arc::new(crate::agent::host_runtime::NativeRuntime::new()),
        std::sync::Arc::new(NodeBootstrap::new(std::sync::Arc::new(
            crate::config::Config::default(),
        ))),
        crate::config::RuntimePoolConfig::default(),
        std::path::PathBuf::from("."),
    );
    assert_eq!(
        tinytools::Tool::parameters_schema(&tool),
        expected_node_exec()
    );
}
