//! One-use user decisions for Jev goal stops, separate from autonomy auto-approval.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};
use tinydesktop_bus::names;

use crate::agent::turn_origin::{self, AgentTurnOrigin};
use crate::config::Config;

const TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Clone)]
struct Pending {
    app: String,
    goal: String,
    origin: String,
    operation: String,
    target: Option<String>,
    target_name: Option<String>,
    target_role: String,
    action_summary: String,
    reason: String,
    expires_at: String,
    created: Instant,
    approved: bool,
}

#[derive(Serialize)]
pub struct PendingDesktopConfirmation {
    pub confirmation_id: String,
    pub app: String,
    pub operation: String,
    pub target: Option<String>,
    pub target_name: Option<String>,
    pub target_role: String,
    pub action_summary: String,
    pub reason: String,
    pub expires_at: String,
    pub approved: bool,
}

fn table() -> &'static Mutex<HashMap<String, Pending>> {
    static TABLE: OnceLock<Mutex<HashMap<String, Pending>>> = OnceLock::new();
    TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn origin_key() -> String {
    match turn_origin::current() {
        Some(AgentTurnOrigin::WebChat { thread_id, .. }) => format!("web:{thread_id}"),
        Some(AgentTurnOrigin::DirectChat) => "direct-chat".to_owned(),
        Some(AgentTurnOrigin::Cli) => "cli".to_owned(),
        _ => "untrusted".to_owned(),
    }
}

/// Capture only the operation and target named by a module confirmation stop.
pub(super) fn record(app: &str, goal: &str, data: &Value) {
    if data.get("stop").and_then(Value::as_str) != Some("confirmation_required") {
        return;
    }
    let Some(id) = data.get("confirmation_id").and_then(Value::as_str) else {
        return;
    };
    let Some(operation) = data
        .get("pending")
        .and_then(|value| value.get("operation"))
        .and_then(Value::as_str)
    else {
        return;
    };
    let target = data
        .get("pending")
        .and_then(|value| value.get("target"))
        .and_then(|value| value.get("ref_id"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let target_name = data
        .get("pending")
        .and_then(|value| value.get("target"))
        .and_then(|value| value.get("name"))
        .and_then(Value::as_str)
        .and_then(|name| {
            (!name.trim().is_empty()).then(|| name.trim().chars().take(100).collect::<String>())
        });
    let target_role = data
        .get("pending")
        .and_then(|value| value.get("target"))
        .and_then(|value| value.get("role"))
        .and_then(Value::as_str)
        .and_then(|role| {
            (!role.trim().is_empty()).then(|| role.trim().chars().take(60).collect::<String>())
        });
    // The raw ref cannot tell a person which control will be activated.
    let (Some(target_name), Some(target_role)) = (target_name, target_role) else {
        return;
    };
    let action_summary = format!("{operation} {target_role} '{target_name}' in {app}");
    let reason = data
        .get("pending")
        .and_then(|value| value.get("reason"))
        .and_then(Value::as_str)
        .unwrap_or("This desktop action needs confirmation.")
        .to_owned();
    let expires_at = (chrono::Utc::now() + chrono::Duration::minutes(10)).to_rfc3339();
    table()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(
            id.to_owned(),
            Pending {
                app: app.to_owned(),
                goal: goal.to_owned(),
                origin: origin_key(),
                operation: operation.to_owned(),
                target,
                target_name: Some(target_name),
                target_role,
                action_summary,
                reason,
                expires_at,
                created: Instant::now(),
                approved: false,
            },
        );
}

pub fn pending() -> Vec<PendingDesktopConfirmation> {
    if !super::ops::listener_is_loopback() {
        return Vec::new();
    }
    let mut guard = table()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard.retain(|_, value| value.created.elapsed() < TTL);
    guard
        .iter()
        .map(|(id, value)| PendingDesktopConfirmation {
            confirmation_id: id.clone(),
            app: value.app.clone(),
            operation: value.operation.clone(),
            target: value.target.clone(),
            approved: value.approved,
            target_name: value.target_name.clone(),
            target_role: value.target_role.clone(),
            action_summary: value.action_summary.clone(),
            reason: value.reason.clone(),
            expires_at: value.expires_at.clone(),
        })
        .collect()
}

/// Called only by a trusted RPC client carrying the core launch bearer.
pub async fn confirm(config: &Config, id: &str, approve: bool) -> Result<Value, String> {
    if !super::ops::listener_is_loopback() {
        return Err("desktop confirmation requires a loopback core listener".to_owned());
    }
    let entry = {
        let mut guard = table()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.retain(|_, value| value.created.elapsed() < TTL);
        let item = guard
            .get_mut(id)
            .ok_or("desktop confirmation is missing or expired")?;
        if approve {
            item.approved = true;
            return Ok(json!({"confirmation_id":id,"approve":true}));
        }
        guard.remove(id).expect("checked above")
    };
    let reply = crate::modules::desktop::call(
        config,
        names::methods::RUN_GOAL,
        json!({"app":entry.app,"goal":entry.goal,
            "continuation":{"id":id,"approve":false}}),
    )
    .await?;
    if !reply.ok {
        return Err(reply.error.map_or_else(
            || "desktop cancellation failed".to_owned(),
            |error| error.message,
        ));
    }
    Ok(json!({"confirmation_id":id,"approve":false}))
}

/// Consume an approved decision before making one continuation call.
pub(super) fn take_approved(id: &str) -> Result<(String, String), String> {
    let mut guard = table()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let item = guard
        .get(id)
        .ok_or("desktop confirmation is missing or expired")?;
    if item.created.elapsed() >= TTL {
        guard.remove(id);
        return Err("desktop confirmation expired".to_owned());
    }
    if !item.approved {
        return Err("desktop action needs the user's explicit confirmation".to_owned());
    }
    if item.origin != origin_key() {
        return Err("desktop confirmation does not match this thread".to_owned());
    }
    let item = guard.remove(id).expect("checked above");
    Ok((item.app, item.goal))
}

#[cfg(test)]
#[path = "confirmation_tests.rs"]
mod tests;
