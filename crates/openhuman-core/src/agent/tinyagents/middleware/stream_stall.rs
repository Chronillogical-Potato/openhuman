//! Abort a provider stream whose visible text has become repetitive narration.

use std::sync::Mutex;

use async_trait::async_trait;
use tinyagents_harness::context::RunContext;
use tinyagents_harness::error::{Result as TaResult, TinyAgentsError};
use tinyagents_harness::middleware::Middleware;
use tinyagents_harness::no_progress::StreamTextStallDetector;
use tinyinference_llm::model::ModelDelta;

use crate::agent::tinyagents::host::OpenHumanRunContext;

pub(crate) const STREAM_STALL_ERROR: &str = "streamed response stalled on repeated narration";

#[derive(Default)]
pub(crate) struct StreamStallMiddleware {
    current: Mutex<(String, StreamTextStallDetector)>,
}

impl StreamStallMiddleware {
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl Middleware<(), OpenHumanRunContext> for StreamStallMiddleware {
    fn name(&self) -> &str {
        "stream_stall"
    }

    async fn on_model_delta(
        &self,
        _ctx: &mut RunContext<OpenHumanRunContext>,
        _state: &(),
        delta: &mut ModelDelta,
    ) -> TaResult<()> {
        let Ok(mut current) = self.current.lock() else {
            return Ok(());
        };
        if current.0 != delta.call_id {
            current.0.clone_from(&delta.call_id);
            current.1.reset();
        }
        if delta.tool_call.is_some() {
            current.1.reset();
            return Ok(());
        }
        if current.1.observe(&delta.content) {
            tracing::warn!("[tinyagents::mw] stopped a repetitive model text stream");
            return Err(TinyAgentsError::Middleware(STREAM_STALL_ERROR.to_string()));
        }
        Ok(())
    }
}
