//! One-shot approval requests for TinySecurity keyring operations.

use std::collections::HashMap;
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

impl PendingConsentRegistry {
    pub(crate) fn register(
        &mut self,
        access: &str,
        name: &str,
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
                expires_at_ms: now_ms().saturating_add(120_000),
            },
            receiver,
        ))
    }

    pub(crate) fn resolve(&mut self, request_id: &str, approved: bool) -> bool {
        self.pending
            .remove(request_id)
            .is_some_and(|sender| sender.send(approved).is_ok())
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
