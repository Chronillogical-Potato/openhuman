use super::*;
use serde_json::json;

fn expected_npm_exec() -> serde_json::Value {
    json!({
            "type": "object",
            "properties": {
                "subcommand": {
                    "type": "string",
                    "description": "npm subcommand, e.g. `install`, `ci`, `run`, `test`, `exec`."
                },
                "args": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Arguments appended after the subcommand (e.g. [\"build\"] for `npm run build`)."
                },
                "cwd": {
                    "type": "string",
                    "description": "Optional sub-directory (relative to workspace) to run npm in. Defaults to the workspace root."
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Optional wall-clock timeout (seconds) before npm is killed. No timeout by default — installs/builds run to completion. Capped at 1800s; 0 disables."
                }
            },
            "required": ["subcommand"]
        })
}

#[test]
fn npm_exec_static_schema_matches_json_literal() {
    let tool = NpmExecTool::new(std::sync::Arc::new(crate::security::SecurityPolicy::default()), std::sync::Arc::new(crate::agent::host_runtime::NativeRuntime::new()), std::sync::Arc::new(NodeBootstrap::new(std::sync::Arc::new(crate::config::Config::default()))));
    assert_eq!(tinytools::Tool::parameters_schema(&tool), expected_npm_exec());
}
