//! Request-scoped delivery of the turn's memory pack.
//!
//! The session host recalls one pack per turn before the model runs
//! (`memory::lifecycle::hooks::pre_turn`) and parks it on the run context
//! (`OpenHumanRunContext::memory_turn`). This middleware adds it to every model
//! request of that turn with [`push_ephemeral_instruction`]: a tail system
//! message, or on a model that hoists system turns (DeepSeek, native
//! Anthropic) a note on the tail user or tool message. The request is built
//! from a copy of the working transcript, so the pack is never committed —
//! the thread's persisted history and the provider's cached prefix stay
//! exactly as they were.

use async_trait::async_trait;

use tinyagents_harness::context::RunContext;
use tinyagents_harness::error::Result as TaResult;
use tinyagents_harness::middleware::{push_ephemeral_instruction, Middleware};
use tinyinference_llm::model::ModelRequest;

use crate::agent::tinyagents::host::OpenHumanRunContext;

/// Adds the turn's memory pack to each of its model requests.
pub(crate) struct MemoryPackMiddleware;

#[async_trait]
impl Middleware<(), OpenHumanRunContext> for MemoryPackMiddleware {
    fn name(&self) -> &str {
        "memory_pack"
    }

    fn is_observer(&self) -> bool {
        true
    }

    async fn before_model(
        &self,
        ctx: &mut RunContext<OpenHumanRunContext>,
        _state: &(),
        request: &mut ModelRequest,
    ) -> TaResult<()> {
        let Some(pack) = ctx
            .data
            .memory_turn
            .as_ref()
            .and_then(|turn| turn.pack.as_ref())
        else {
            return Ok(());
        };
        let injection = pack.injection();
        tracing::trace!(
            tokens = pack.tokens,
            refs = pack.refs.len(),
            "[tinyagents::mw] memory pack added to the model request"
        );
        push_ephemeral_instruction(request, injection, ctx.model_profile.as_ref());
        Ok(())
    }
}

#[cfg(test)]
#[path = "memory_pack_tests.rs"]
mod tests;
