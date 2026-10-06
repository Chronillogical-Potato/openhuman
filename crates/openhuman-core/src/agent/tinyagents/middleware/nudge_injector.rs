//! Request-scoped delivery of `RepeatedToolFailureMiddleware` nudges.
//!
//! Split from `repeated_failure.rs` to stay under the Rust layout line limit.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use tinyagents_harness::context::RunContext;
use tinyagents_harness::error::Result as TaResult;
use tinyagents_harness::middleware::{push_ephemeral_instruction, Middleware};
use tinyinference_llm::model::ModelRequest;

/// Appends queued `RepeatedToolFailureMiddleware` nudges to the next model
/// request, then forgets them. The request is built from a copy of the working
/// transcript, so nothing it adds is ever committed.
/// [`push_ephemeral_instruction`] places them: a tail system message, except on
/// a model that hoists system turns (DeepSeek), where one resets the prompt
/// cache (#6962), so the nudge rides the tail tool result.
pub(crate) struct PendingNudgeInjector {
    pub(super) pending: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl Middleware<(), crate::agent::tinyagents::host::OpenHumanRunContext> for PendingNudgeInjector {
    fn name(&self) -> &str {
        "pending_nudge_injector"
    }

    // A prior middleware can request retry/stop control after a failed tool
    // call. The queued correction still has to reach that retry request.
    fn is_observer(&self) -> bool {
        true
    }

    async fn before_model(
        &self,
        ctx: &mut RunContext<crate::agent::tinyagents::host::OpenHumanRunContext>,
        _state: &(),
        request: &mut ModelRequest,
    ) -> TaResult<()> {
        let nudges = self
            .pending
            .lock()
            .map(|mut pending| std::mem::take(&mut *pending))
            .unwrap_or_default();
        if !nudges.is_empty() {
            let hoists = ctx
                .model_profile
                .as_ref()
                .is_some_and(|profile| profile.hoists_system_messages);
            tracing::debug!(
                count = nudges.len(),
                hoists,
                "[tinyagents::mw] request-scoped nudge(s) appended to the next model request"
            );
            for nudge in nudges {
                push_ephemeral_instruction(request, nudge, ctx.model_profile.as_ref());
            }
        }
        Ok(())
    }
}
