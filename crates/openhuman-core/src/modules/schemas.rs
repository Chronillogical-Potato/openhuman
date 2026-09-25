//! The `modules` RPC namespace.
//!
//! Read-only plus one deliberate action. `list` and `status` report what this
//! build knows and what it has loaded; `load` forces a lazy module to resolve
//! now, which is what a settings screen offering "install this now" needs.
//!
//! There is no `unload`, and there cannot be: tinybus never unloads a library.
//! There is also no way to name an artifact over RPC — the loadable set is
//! compiled into [`super::registry`], and a method that could point the loader at
//! an arbitrary path would turn this namespace into remote code execution.

use serde_json::{Map, Value};
use std::sync::Arc;
use tinybrowser_bus::SessionOptions;

use super::ops;
use crate::config::rpc as config_rpc;
use crate::core::all::{ControllerFuture, RegisteredController};
use crate::core::{ControllerSchema, FieldSchema, TypeSchema};

pub fn all_controller_schemas() -> Vec<ControllerSchema> {
    vec![
        schemas("list"),
        schemas("status"),
        schemas("load"),
        schemas("browser_check_readiness"),
    ]
}

pub fn all_registered_controllers() -> Vec<RegisteredController> {
    vec![
        RegisteredController {
            schema: schemas("list"),
            handler: handle_list,
        },
        RegisteredController {
            schema: schemas("status"),
            handler: handle_status,
        },
        RegisteredController {
            schema: schemas("load"),
            handler: handle_load,
        },
        RegisteredController {
            schema: schemas("browser_check_readiness"),
            handler: handle_browser_check_readiness,
        },
    ]
}

pub fn schemas(function: &str) -> ControllerSchema {
    match function {
        "list" => ControllerSchema {
            namespace: "modules",
            function: "list",
            description: "List every loadable module this build knows, with its state.",
            inputs: vec![],
            outputs: vec![FieldSchema {
                name: "modules",
                ty: TypeSchema::Array(Box::new(TypeSchema::Ref("ModuleStatus"))),
                comment: "Status of each known module.",
                required: true,
            }],
        },
        "status" => ControllerSchema {
            namespace: "modules",
            function: "status",
            description: "Report the state of one module by id.",
            inputs: vec![FieldSchema {
                name: "id",
                ty: TypeSchema::String,
                comment: "Registry identifier, e.g. `tinydocs`.",
                required: true,
            }],
            outputs: vec![FieldSchema {
                name: "module",
                ty: TypeSchema::Ref("ModuleStatus"),
                comment: "Status of the requested module.",
                required: true,
            }],
        },
        "load" => ControllerSchema {
            namespace: "modules",
            function: "load",
            description: "Resolve and load a module now instead of on first use.",
            inputs: vec![FieldSchema {
                name: "id",
                ty: TypeSchema::String,
                comment: "Registry identifier, e.g. `tinydocs`.",
                required: true,
            }],
            outputs: vec![FieldSchema {
                name: "module",
                ty: TypeSchema::Ref("ModuleStatus"),
                comment: "Status after the load attempt.",
                required: true,
            }],
        },
        "browser_check_readiness" => ControllerSchema {
            namespace: "modules",
            function: "browser_check_readiness",
            description: "Load TinyBrowser and briefly launch Chrome to check readiness.",
            inputs: vec![],
            outputs: vec![
                FieldSchema {
                    name: "module_ready",
                    ty: TypeSchema::Bool,
                    comment: "TinyBrowser module is serving.",
                    required: true,
                },
                FieldSchema {
                    name: "chrome_ready",
                    ty: TypeSchema::Bool,
                    comment: "Chrome launched and closed successfully.",
                    required: true,
                },
                FieldSchema {
                    name: "error",
                    ty: TypeSchema::Option(Box::new(TypeSchema::String)),
                    comment: "Sanitized setup failure, if any.",
                    required: false,
                },
            ],
        },
        _ => ControllerSchema {
            namespace: "modules",
            function: "unknown",
            description: "Unknown modules controller function.",
            inputs: vec![],
            outputs: vec![FieldSchema {
                name: "error",
                ty: TypeSchema::String,
                comment: "Lookup error details.",
                required: true,
            }],
        },
    }
}

fn handle_browser_check_readiness(_params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let mut config = config_rpc::load_config_with_timeout().await?;
        // The probe never navigates. Give its disposable session a non-routable
        // origin so an empty website allowlist does not prevent a Chrome check.
        config.http_request.allowed_domains = vec!["example.invalid".into()];
        let client = super::browser::BrowserClient::new(Arc::new(config));
        if client.ensure_ready().await.is_err() {
            return Ok(
                serde_json::json!({"module_ready": false, "chrome_ready": false, "error": "TinyBrowser module is unavailable; configure a local module override"}),
            );
        }
        let opened = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            client.open_session(SessionOptions::default()),
        )
        .await;
        match opened {
            Ok(Ok(session)) => {
                // A session is always closed after this non-navigating probe.
                let closed = client.close_session(&session.id).await.is_ok();
                Ok(
                    serde_json::json!({"module_ready": true, "chrome_ready": closed,
                    "error": if closed { None } else { Some("Chrome session could not close cleanly") }}),
                )
            }
            Ok(Err(_)) => Ok(
                serde_json::json!({"module_ready": true, "chrome_ready": false,
                "error": "Chrome could not start; check browser settings and allowed websites"}),
            ),
            Err(_) => Ok(
                serde_json::json!({"module_ready": true, "chrome_ready": false,
                "error": "Chrome launch timed out"}),
            ),
        }
    })
}

fn handle_list(_params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let config = config_rpc::load_config_with_timeout().await?;
        Ok(serde_json::json!({ "modules": ops::list(&config) }))
    })
}

fn handle_status(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let id = string_param(&params, "id").ok_or("`id` is required")?;
        let config = config_rpc::load_config_with_timeout().await?;
        match ops::list(&config).into_iter().find(|m| m.id == id) {
            Some(module) => Ok(serde_json::json!({ "module": module })),
            None => Err(format!("unknown module '{id}'")),
        }
    })
}

fn handle_load(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let id = string_param(&params, "id").ok_or("`id` is required")?;
        let config = config_rpc::load_config_with_timeout().await?;
        // A failed load is reported through the returned status rather than as
        // an RPC error: the caller asked "what happened", and the answer is a
        // state plus a reason, not a transport failure.
        if let Err(reason) = ops::ensure_loaded(&config, &id).await {
            log::warn!("[modules] explicit load of '{id}' failed: {reason}");
        }
        match ops::list(&config).into_iter().find(|m| m.id == id) {
            Some(module) => Ok(serde_json::json!({ "module": module })),
            None => Err(format!("unknown module '{id}'")),
        }
    })
}

/// A required string parameter, rejecting a blank one.
fn string_param(params: &Map<String, Value>, key: &str) -> Option<String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
#[path = "schemas_tests.rs"]
mod tests;
