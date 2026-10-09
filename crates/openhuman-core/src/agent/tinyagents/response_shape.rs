//! Per-turn response shape for a library host's turn: a response format and
//! an output cap applied to every model call of the turn, and a report of the
//! final call (finish reason, answering model, reasoning tokens).
//!
//! Carried as a task-local rather than through the session, because the
//! session host's turn plumbing is shared by every product path and none of
//! them need this. `inference::host_runtime::ops::agent_chat_reply_for` scopes
//! it around the turn for an `AgentChatTarget::Definition` that asked for it;
//! the turn runner reads it **once**, on the turn's own task, when it builds a
//! root turn's harness ([`install`]), and the middleware holds the `Arc` from
//! then on, so a model call that runs on another task still sees it.
//! Sub-agent turns never install it: the shape is the host's statement about
//! *its* turn's answer.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tinyagents_harness::context::RunContext;
use tinyagents_harness::error::Result as TaResult;
use tinyagents_harness::middleware::Middleware;
use tinyagents_harness::runtime::AgentHarness;
use tinyinference_llm::model::{ModelRequest, ModelResponse, ResponseFormat};

use crate::agent::tinyagents::host::OpenHumanRunContext;

/// What a host asks of every model call in one turn.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResponseShape {
    /// Sent as `response_format` on every call of the turn's tool loop.
    pub response_format: Option<ResponseFormat>,
    /// Replaces the turn's per-call output cap.
    pub max_output_tokens: Option<u32>,
}

/// What the turn's final model call reported.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FinalResponse {
    /// The provider's finish reason for the last call (`stop`, `length`, ...).
    pub finish_reason: Option<String>,
    /// The model the provider says answered the last call.
    pub answered_model: Option<String>,
    /// Reasoning tokens summed over every call of the turn.
    pub reasoning_tokens: u64,
}

/// A shape and the report slot its turn fills.
#[derive(Debug, Default)]
pub struct ResponseShapeScope {
    shape: ResponseShape,
    report: Mutex<FinalResponse>,
}

impl ResponseShapeScope {
    /// A scope asking for `shape`.
    #[must_use]
    pub fn new(shape: ResponseShape) -> Arc<Self> {
        Arc::new(Self {
            shape,
            report: Mutex::default(),
        })
    }

    /// What the turn's calls have reported so far.
    #[must_use]
    pub fn report(&self) -> FinalResponse {
        self.report
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

tokio::task_local! {
    static RESPONSE_SHAPE: Arc<ResponseShapeScope>;
}

/// Run `fut` with `scope` as its turn's response shape.
pub async fn with_response_shape<F: std::future::Future>(
    scope: Arc<ResponseShapeScope>,
    fut: F,
) -> F::Output {
    RESPONSE_SHAPE.scope(scope, Box::pin(fut)).await
}

/// Push the shaping middleware onto a root turn's harness, when a shape is
/// scoped around the turn.
pub(super) fn install(harness: &mut AgentHarness<(), OpenHumanRunContext>, root: bool) {
    if !root {
        return;
    }
    if let Ok(scope) = RESPONSE_SHAPE.try_with(Arc::clone) {
        log::debug!(
            "[tinyagents] response shape installed: format={} max_output_tokens={:?}",
            scope.shape.response_format.is_some(),
            scope.shape.max_output_tokens
        );
        harness.push_middleware(Arc::new(ResponseShapeMiddleware(scope)));
    }
}

struct ResponseShapeMiddleware(Arc<ResponseShapeScope>);

#[async_trait]
impl Middleware<(), OpenHumanRunContext> for ResponseShapeMiddleware {
    fn name(&self) -> &str {
        "openhuman_response_shape"
    }

    async fn before_model(
        &self,
        _ctx: &mut RunContext<OpenHumanRunContext>,
        _state: &(),
        request: &mut ModelRequest,
    ) -> TaResult<()> {
        let shape = &self.0.shape;
        if let Some(format) = &shape.response_format {
            request.response_format = Some(format.clone());
        }
        if let Some(cap) = shape.max_output_tokens {
            request.max_tokens = Some(cap);
        }
        Ok(())
    }

    async fn after_model(
        &self,
        _ctx: &mut RunContext<OpenHumanRunContext>,
        _state: &(),
        response: &mut ModelResponse,
    ) -> TaResult<()> {
        record(&self.0, response);
        Ok(())
    }
}

/// Fold one completed call into the report: the last call's finish reason
/// and model win; reasoning tokens add up (a cache replay spent none).
fn record(scope: &ResponseShapeScope, response: &ModelResponse) {
    let mut report = scope
        .report
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    report.finish_reason = response.finish_reason.clone();
    report.answered_model = response
        .resolved_route
        .as_ref()
        .map(|route| route.model.clone())
        .or_else(|| response.resolved_model.as_ref().map(|m| m.name.clone()))
        .filter(|model| !model.trim().is_empty());
    if !response.served_from_cache {
        if let Some(usage) = &response.usage {
            report.reasoning_tokens += usage.reasoning_tokens;
        }
    }
}

#[cfg(test)]
#[path = "response_shape_tests.rs"]
mod tests;
