//! Deferred desktop tools. Only their names and descriptions enter discovery.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use tinydesktop_bus::{
    names, FindRequest, ListAppsRequest, ListWindowsRequest, RefRequest, SnapshotRequest,
    TypeRequest,
};
use tinytools::{PermissionLevel, Tool, ToolExposure, ToolResult};

use crate::config::Config;

#[derive(Clone, Copy)]
pub enum DesktopToolKind {
    Apps,
    Windows,
    Snapshot,
    Find,
    Act,
    Goal,
}

pub struct DesktopTool {
    config: Arc<Config>,
    kind: DesktopToolKind,
}

impl DesktopTool {
    pub fn new(config: Arc<Config>, kind: DesktopToolKind) -> Self {
        Self { config, kind }
    }
}

fn required(args: &Value, key: &str) -> anyhow::Result<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("missing required parameter: {key}"))
}

#[async_trait]
impl Tool for DesktopTool {
    fn name(&self) -> &str {
        match self.kind {
            DesktopToolKind::Apps => "desktop_list_apps",
            DesktopToolKind::Windows => "desktop_list_windows",
            DesktopToolKind::Snapshot => "desktop_snapshot",
            DesktopToolKind::Find => "desktop_find",
            DesktopToolKind::Act => "desktop_act",
            DesktopToolKind::Goal => "desktop_goal",
        }
    }

    fn description(&self) -> &str {
        match self.kind {
            DesktopToolKind::Apps => "List running desktop applications on this computer.",
            DesktopToolKind::Windows => "List native windows for a running desktop app.",
            DesktopToolKind::Snapshot => "Inspect an app's accessibility tree and obtain snapshot-qualified element refs.",
            DesktopToolKind::Find => "Find a desktop accessibility element by role and name, returning a ref.",
            DesktopToolKind::Act => "Act on a desktop element ref: click, focus, type, check, uncheck, expand, or collapse.",
            DesktopToolKind::Goal => "Run a bounded Jev-guided desktop goal. Search for desktop tools before use; consequential actions require confirmation.",
        }
    }

    fn exposure(&self) -> ToolExposure {
        ToolExposure::Deferred
    }

    fn permission_level(&self) -> PermissionLevel {
        match self.kind {
            DesktopToolKind::Act | DesktopToolKind::Goal => PermissionLevel::Write,
            _ => PermissionLevel::ReadOnly,
        }
    }

    fn external_effect(&self) -> bool {
        matches!(self.kind, DesktopToolKind::Act | DesktopToolKind::Goal)
    }

    fn parameters_schema(&self) -> Value {
        match self.kind {
            DesktopToolKind::Apps => json!({"type":"object","properties":{}}),
            DesktopToolKind::Windows => json!({"type":"object","properties":{
                "app":{"type":"string","description":"Optional application name"}}}),
            DesktopToolKind::Snapshot => json!({"type":"object","properties":{
                "app":{"type":"string"}, "skeleton":{"type":"boolean"},
                "root_ref":{"type":"string"}, "max_depth":{"type":"integer","minimum":1,"maximum":12}}}),
            DesktopToolKind::Find => json!({"type":"object","properties":{
                "app":{"type":"string"},"role":{"type":"string"},"name":{"type":"string"},
                "root":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":20}}}),
            DesktopToolKind::Act => json!({"type":"object","properties":{
                "operation":{"type":"string","enum":["click","focus","type","check","uncheck","expand","collapse"]},
                "ref_id":{"type":"string","description":"Snapshot-qualified ref from desktop_snapshot or desktop_find"},
                "text":{"type":"string","description":"Required for type"}},
                "required":["operation","ref_id"]}),
            DesktopToolKind::Goal => json!({"type":"object","properties":{
                "app":{"type":"string"},"goal":{"type":"string"},
                "text":{"type":"array","items":{"type":"string"}},
                "max_steps":{"type":"integer","minimum":1,"maximum":8},
                "max_model_calls":{"type":"integer","minimum":1,"maximum":16},
                "confirmation_id":{"type":"string","description":"Continue only after the user approved this pending action in the desktop confirmation UI"}}}),
        }
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        if !super::ops::listener_is_loopback() || !super::ops::enabled(&self.config) {
            return Ok(ToolResult::error(
                "Desktop control is disabled in Connections.",
            ));
        }
        let permission = crate::modules::desktop::permissions(&self.config).await;
        let permission = match permission {
            Ok(value) if value.ok => value,
            Ok(value) => {
                return Ok(ToolResult::error(format!(
                    "Desktop permission check failed: {}",
                    value
                        .error
                        .map_or_else(|| "unknown error".to_owned(), |error| error.message)
                )))
            }
            Err(error) => return Ok(ToolResult::error(error)),
        };
        let accessibility = permission
            .data
            .as_ref()
            .and_then(|data| data.get("accessibility"))
            .and_then(|value| value.get("state"))
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if accessibility != "granted"
            && !matches!(self.kind, DesktopToolKind::Apps | DesktopToolKind::Windows)
        {
            return Ok(ToolResult::error("Desktop Accessibility permission is not granted. Enable it in system settings and retry."));
        }
        let mut goal_identity = None;
        let (member, request) = match self.kind {
            DesktopToolKind::Apps => (
                names::methods::LIST_APPS,
                serde_json::to_value(ListAppsRequest::default())?,
            ),
            DesktopToolKind::Windows => (
                names::methods::LIST_WINDOWS,
                serde_json::to_value(ListWindowsRequest {
                    app: args.get("app").and_then(Value::as_str).map(str::to_owned),
                })?,
            ),
            DesktopToolKind::Snapshot => {
                let request = SnapshotRequest {
                    app: args.get("app").and_then(Value::as_str).map(str::to_owned),
                    skeleton: args
                        .get("skeleton")
                        .and_then(Value::as_bool)
                        .unwrap_or(true),
                    root_ref: args
                        .get("root_ref")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    max_depth: args
                        .get("max_depth")
                        .and_then(Value::as_u64)
                        .map(|n| n.clamp(1, 12) as u8),
                    ..SnapshotRequest::default()
                };
                (names::methods::SNAPSHOT, serde_json::to_value(request)?)
            }
            DesktopToolKind::Find => {
                let request = FindRequest {
                    app: args.get("app").and_then(Value::as_str).map(str::to_owned),
                    role: args.get("role").and_then(Value::as_str).map(str::to_owned),
                    name: args.get("name").and_then(Value::as_str).map(str::to_owned),
                    root: args.get("root").and_then(Value::as_str).map(str::to_owned),
                    limit: Some(
                        args.get("limit")
                            .and_then(Value::as_u64)
                            .unwrap_or(10)
                            .clamp(1, 20) as usize,
                    ),
                    ..FindRequest::default()
                };
                (names::methods::FIND, serde_json::to_value(request)?)
            }
            DesktopToolKind::Act => {
                let reference = required(&args, "ref_id")?;
                let operation = required(&args, "operation")?;
                if operation == "type" {
                    let text = required(&args, "text")?;
                    (
                        names::methods::TYPE,
                        serde_json::to_value(TypeRequest {
                            ref_id: reference,
                            text,
                            ..TypeRequest::default()
                        })?,
                    )
                } else {
                    let member = match operation.as_str() {
                        "click" => names::methods::CLICK,
                        "focus" => names::methods::FOCUS,
                        "check" => names::methods::CHECK,
                        "uncheck" => names::methods::UNCHECK,
                        "expand" => names::methods::EXPAND,
                        "collapse" => names::methods::COLLAPSE,
                        _ => return Ok(ToolResult::error("Unsupported desktop operation")),
                    };
                    (member, serde_json::to_value(RefRequest::new(reference))?)
                }
            }
            DesktopToolKind::Goal => {
                let (app, goal, continuation) =
                    if let Some(id) = args.get("confirmation_id").and_then(Value::as_str) {
                        let (app, goal) =
                            super::confirmation::take_approved(id).map_err(anyhow::Error::msg)?;
                        (app, goal, Some(json!({"id":id,"approve":true})))
                    } else {
                        (required(&args, "app")?, required(&args, "goal")?, None)
                    };
                goal_identity = Some((app.clone(), goal.clone()));
                let mut request = json!({"app":app,"goal":goal,
                    "text":args.get("text").cloned().unwrap_or_else(|| json!([])),
                    "include_values":false,
                    "max_steps":args.get("max_steps").and_then(Value::as_u64).unwrap_or(6).clamp(1,8),
                    "max_model_calls":args.get("max_model_calls").and_then(Value::as_u64).unwrap_or(12).clamp(1,16)});
                if let Some(continuation) = continuation {
                    request["continuation"] = continuation;
                }
                (names::methods::RUN_GOAL, request)
            }
        };
        let reply = crate::modules::desktop::call(&self.config, member, request).await;
        match reply {
            Ok(response) if response.ok => {
                let data = response.data.unwrap_or(Value::Null);
                if let Some((app, goal)) = goal_identity {
                    super::confirmation::record(&app, &goal, &data);
                }
                let rendered = serde_json::to_string(&data)?;
                // A bounded model result; full screenshots are not exposed through this tool.
                Ok(ToolResult::success(
                    rendered.chars().take(24_000).collect::<String>(),
                ))
            }
            Ok(response) => Ok(ToolResult::error(response.error.map_or_else(
                || "Desktop command failed without an error".to_owned(),
                |error| format!("{}: {}", error.code, error.message),
            ))),
            Err(error) => Ok(ToolResult::error(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn deferred_tool_is_discoverable_but_fails_closed_when_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = Config::default();
        config.workspace_dir = dir.path().to_path_buf();
        let tool = DesktopTool::new(Arc::new(config), DesktopToolKind::Apps);
        assert_eq!(tool.exposure(), ToolExposure::Deferred);
        let result = tool.execute(json!({})).await.unwrap();
        assert!(result.output().contains("disabled in Connections"));
    }
}
