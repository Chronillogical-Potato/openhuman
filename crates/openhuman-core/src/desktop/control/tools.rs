//! Deferred desktop tools. Only their names and descriptions enter discovery.

use std::future::Future;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use tinydesktop_bus::{
    names, DesktopResponse, FindRequest, LaunchRequest, ListAppsRequest, ListWindowsRequest,
    RefRequest, SnapshotRequest, TypeRequest,
};
use tinytools::{PermissionLevel, Tool, ToolCallOptions, ToolExposure, ToolResult, ToolRunContext};

use crate::config::Config;

#[derive(Clone, Copy)]
pub enum DesktopToolKind {
    Apps,
    Windows,
    Launch,
    Snapshot,
    Find,
    Act,
    Goal,
    ContinueGoal,
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

fn confirmation_id(response: &DesktopResponse) -> Result<Option<String>, String> {
    if !response.ok {
        return Ok(None);
    }
    let Some(data) = response.data.as_ref() else {
        return Ok(None);
    };
    if data.get("stop").and_then(Value::as_str) != Some("confirmation_required") {
        return Ok(None);
    }
    data.get("confirmation_id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .map(Some)
        .ok_or_else(|| {
            "desktop goal requested confirmation without a continuation handle".to_owned()
        })
}

/// The module owns remaining action/model budgets and revalidates a one-use
/// target against a fresh snapshot on every continuation. The host adds a hard
/// eight-confirmation ceiling so a module bug cannot hold this tool forever.
async fn advance_goal_confirmations<F, Fut>(
    mut response: DesktopResponse,
    approvals_enabled: bool,
    mut continue_with: F,
) -> Result<DesktopResponse, String>
where
    F: FnMut(String, bool) -> Fut,
    Fut: Future<Output = Result<DesktopResponse, String>>,
{
    if approvals_enabled {
        return Ok(response);
    }
    for _ in 0..8 {
        let Some(id) = confirmation_id(&response)? else {
            return Ok(response);
        };
        tracing::info!(
            "[desktop] continuing module-confirmed action under approvals-disabled policy"
        );
        response = continue_with(id, true).await?;
    }
    if let Some(id) = confirmation_id(&response)? {
        let executed = response
            .data
            .as_ref()
            .and_then(|data| data.get("turns"))
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let cancelled = continue_with(id, false).await.map_err(|error| {
            format!(
                "desktop goal reached the confirmation continuation limit after {executed} \
             executed action(s); cancellation delivery is uncertain: {error}"
            )
        })?;
        if !cancelled.ok {
            return Err(format!(
                "desktop goal reached the confirmation continuation limit after {executed} \
                 executed action(s); module refused cancellation"
            ));
        }
        return Ok(cancelled);
    }
    Ok(response)
}

#[async_trait]
impl Tool for DesktopTool {
    fn name(&self) -> &str {
        match self.kind {
            DesktopToolKind::Apps => "desktop_list_apps",
            DesktopToolKind::Windows => "desktop_list_windows",
            DesktopToolKind::Launch => "desktop_launch",
            DesktopToolKind::Snapshot => "desktop_snapshot",
            DesktopToolKind::Find => "desktop_find",
            DesktopToolKind::Act => "desktop_act",
            DesktopToolKind::Goal => "desktop_goal",
            DesktopToolKind::ContinueGoal => "desktop_continue_goal",
        }
    }

    fn description(&self) -> &str {
        match self.kind {
            DesktopToolKind::Apps => "List running desktop applications on this computer.",
            DesktopToolKind::Windows => "List native windows for a running desktop app.",
            DesktopToolKind::Launch => "Launch or activate a named desktop app so its window becomes available.",
            DesktopToolKind::Snapshot => "Inspect an app's accessibility tree and obtain snapshot-qualified element refs.",
            DesktopToolKind::Find => "Find a desktop accessibility element by role and name, returning a ref.",
            DesktopToolKind::Act => "Act on a desktop element ref: click, focus, type, check, uncheck, expand, or collapse.",
            DesktopToolKind::Goal => "Run a bounded Jev-guided desktop goal. Search for desktop tools before use; consequential actions require confirmation.",
            DesktopToolKind::ContinueGoal => "Resume a desktop goal after the user approved its exact pending action in Connections.",
        }
    }

    fn exposure(&self) -> ToolExposure {
        ToolExposure::Deferred
    }

    fn family(&self) -> Option<&str> {
        Some("desktop")
    }

    fn permission_level(&self) -> PermissionLevel {
        match self.kind {
            DesktopToolKind::Act
            | DesktopToolKind::Goal
            | DesktopToolKind::ContinueGoal
            | DesktopToolKind::Launch => PermissionLevel::Write,
            _ => PermissionLevel::ReadOnly,
        }
    }

    fn external_effect(&self) -> bool {
        matches!(
            self.kind,
            DesktopToolKind::Act
                | DesktopToolKind::Goal
                | DesktopToolKind::ContinueGoal
                | DesktopToolKind::Launch
        )
    }

    fn parameters_schema(&self) -> Value {
        match self.kind {
            DesktopToolKind::Apps => json!({"type":"object","properties":{}}),
            DesktopToolKind::Windows => json!({"type":"object","properties":{
                "app":{"type":"string","description":"Optional application name"}}}),
            DesktopToolKind::Launch => json!({"type":"object","properties":{
                "app":{"type":"string","description":"Application name, e.g. Spotify or TextEdit"}},
                "required":["app"],"additionalProperties":false}),
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
                "max_model_calls":{"type":"integer","minimum":1,"maximum":16}},
                "required":["app","goal"]}),
            DesktopToolKind::ContinueGoal => json!({"type":"object","properties":{
                "confirmation_id":{"type":"string","description":"One-use handle approved by the user in Connections"}},
                "required":["confirmation_id"]}),
        }
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        self.execute_with_context(args, ToolCallOptions::default(), None)
            .await
    }

    async fn execute_with_context(
        &self,
        args: Value,
        _options: ToolCallOptions,
        context: Option<&dyn ToolRunContext>,
    ) -> anyhow::Result<ToolResult> {
        let thread_id = context
            .and_then(ToolRunContext::thread_id)
            .filter(|thread_id| !thread_id.is_empty());
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
            && !matches!(
                self.kind,
                DesktopToolKind::Apps | DesktopToolKind::Windows | DesktopToolKind::Launch
            )
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
            DesktopToolKind::Launch => {
                let mut request = LaunchRequest::new(required(&args, "app")?);
                request.activate = true;
                request.attach_if_running = Some(true);
                (names::methods::LAUNCH, serde_json::to_value(request)?)
            }
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
            DesktopToolKind::Goal | DesktopToolKind::ContinueGoal => {
                let (app, goal, continuation) =
                    if matches!(self.kind, DesktopToolKind::ContinueGoal) {
                        let id = required(&args, "confirmation_id")?;
                        let (app, goal) = super::confirmation::take_approved(&id, thread_id)
                            .map_err(anyhow::Error::msg)?;
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
        let goal_action = matches!(
            self.kind,
            DesktopToolKind::Goal | DesktopToolKind::ContinueGoal
        );
        let approvals_enabled = if goal_action {
            let live_config = match crate::config::rpc::load_config_with_timeout().await {
                Ok(config) => config,
                Err(error) => {
                    return Ok(ToolResult::error(format!(
                        "Desktop approval setting is unavailable: {error}"
                    )))
                }
            };
            live_config.desktop.approvals_enabled
        } else {
            true
        };
        let reply = crate::modules::desktop::call(&self.config, member, request).await;
        let reply = if goal_action {
            match reply {
                Ok(response) => {
                    let config = Arc::clone(&self.config);
                    advance_goal_confirmations(response, approvals_enabled, move |id, approve| {
                        let config = Arc::clone(&config);
                        async move {
                            crate::modules::desktop::call(
                                &config,
                                names::methods::RUN_GOAL,
                                json!({"continuation":{"id":id,"approve":approve}}),
                            )
                            .await
                        }
                    })
                    .await
                }
                Err(error) => Err(error),
            }
        } else {
            reply
        };
        match reply {
            Ok(response) if response.ok => {
                let data = response.data.unwrap_or(Value::Null);
                if let Some((app, goal)) = goal_identity.filter(|_| approvals_enabled) {
                    if data.get("stop").and_then(Value::as_str) == Some("confirmation_required") {
                        let Some(thread_id) = thread_id else {
                            return Ok(ToolResult::error(
                                "desktop confirmation requires a threaded agent run",
                            ));
                        };
                        super::confirmation::record(&app, &goal, thread_id, &data);
                    }
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
#[path = "tools_tests.rs"]
mod tests;
