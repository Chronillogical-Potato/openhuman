//! One-shot approval requests for TinySecurity keyring operations.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tokio::sync::oneshot;

pub(crate) const MAX_PENDING_APPROVALS: usize = 64;

#[derive(Debug, Clone)]
pub(crate) struct ApprovalPrompt {
    pub request_id: String,
    pub access: String,
    pub name: String,
    pub expires_at_ms: u64,
}

#[derive(Default)]
pub(crate) struct PendingConsentRegistry {
    pending: HashMap<String, oneshot::Sender<bool>>,
}

static PENDING: OnceLock<Mutex<PendingConsentRegistry>> = OnceLock::new();

fn pending() -> &'static Mutex<PendingConsentRegistry> {
    PENDING.get_or_init(|| Mutex::new(PendingConsentRegistry::default()))
}

impl PendingConsentRegistry {
    pub(crate) fn register(
        &mut self,
        access: &str,
        name: &str,
        timeout: std::time::Duration,
    ) -> Result<(ApprovalPrompt, oneshot::Receiver<bool>), &'static str> {
        if self.pending.len() >= MAX_PENDING_APPROVALS {
            return Err("too many pending keyring consent requests");
        }

        let (sender, receiver) = oneshot::channel();
        let request_id = uuid::Uuid::new_v4().to_string();
        self.pending.insert(request_id.clone(), sender);
        Ok((
            ApprovalPrompt {
                request_id,
                access: access.to_owned(),
                name: name.to_owned(),
                expires_at_ms: now_ms()
                    .saturating_add(timeout.as_millis().try_into().unwrap_or(u64::MAX)),
            },
            receiver,
        ))
    }

    pub(crate) fn resolve(&mut self, request_id: &str, approved: bool) -> bool {
        self.pending
            .remove(request_id)
            .is_some_and(|sender| sender.send(approved).is_ok())
    }

    pub(crate) fn cancel(&mut self, request_id: &str) {
        self.pending.remove(request_id);
    }
}

/// Request approval for this exact operation and wait for its correlated
/// response. Missing event infrastructure, a full queue, timeout, or a lost
/// waiter all fail closed.
pub(crate) async fn request_approval(
    request: tinysecurity_bus::KeyringConsentRequest2,
) -> Option<bool> {
    use tinysecurity_bus::KeyringAccess2;

    let access = match request.access {
        KeyringAccess2::Read => "read",
        KeyringAccess2::Write => "write",
        KeyringAccess2::Delete => "delete",
    };
    let timeout = std::time::Duration::from_millis(u64::from(request.timeout_ms.min(120_000)));
    let (prompt, waiter) = pending()
        .lock()
        .ok()?
        .register(access, &request.name, timeout)
        .ok()?;
    crate::core::bus::BUS.publish(
        crate::core::events::DomainEvent::KeyringAccessApprovalRequested {
            request_id: prompt.request_id.clone(),
            caller_agent: request.caller.agent.clone(),
            origin: format!("{:?}", request.caller.origin),
            purpose: request.purpose,
            access: prompt.access,
            name: prompt.name,
            expires_at_ms: prompt.expires_at_ms,
        },
    );

    await_decision(pending(), &prompt.request_id, waiter, timeout).await
}

async fn await_decision(
    registry: &Mutex<PendingConsentRegistry>,
    request_id: &str,
    waiter: oneshot::Receiver<bool>,
    timeout: std::time::Duration,
) -> Option<bool> {
    match tokio::time::timeout(timeout, waiter).await {
        Ok(Ok(decision)) => Some(decision),
        Ok(Err(_)) | Err(_) => {
            if let Ok(mut pending) = registry.lock() {
                pending.cancel(request_id);
            }
            None
        }
    }
}

/// Resolve one currently pending request. Each id can be consumed once.
pub(crate) fn resolve_approval(request_id: &str, approved: bool) -> bool {
    pending()
        .lock()
        .is_ok_and(|mut pending| pending.resolve(request_id, approved))
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "operation_approval_tests.rs"]
mod tests;
