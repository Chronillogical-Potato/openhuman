//! Boxed entry to the tinyagents turn body (see [`run_turn_via_tinyagents_inner`]).
//! Kept beside `turn_runner.rs` so that file stays under the layout line limit.

use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;
use tinyagents_harness::run_queue::RunQueue;
use tinyagents_session::transcript::TranscriptMessage;

use crate::agent::tinyagents::host::OpenHumanRunContext;
use crate::agent::tinyagents::middleware::TurnContextMiddleware;
use crate::agent::tinyagents::observability::SubagentScope;
use crate::agent::tinyagents::turn_models::TurnModels;
use crate::agent::tinyagents::turn_outcome::TinyagentsTurnOutcome;

use super::turn_runner::run_turn_via_tinyagents_body;
use super::ToolPolicyEnforcement;

/// Returned boxed and `#[inline(never)]` on purpose: an `async fn` body is
/// otherwise re-instantiated inside every crate / codegen unit that awaits it,
/// and this state machine is large. Boxing here keeps one copy, compiled in
/// this crate.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(super) fn run_turn_via_tinyagents_inner<'a>(
    run_context: OpenHumanRunContext,
    turn_models: TurnModels,
    provider_id: String,
    model: &'a str,
    history: Vec<TranscriptMessage>,
    tool_sets: Vec<Arc<Vec<Box<dyn tinytools::Tool>>>>,
    allowed: Option<HashSet<String>>,
    max_iterations: usize,
    subagent_scope: Option<SubagentScope>,
    context_window: Option<u64>,
    run_queue: Option<Arc<RunQueue<crate::agent::queued_turn::QueuedTurn>>>,
    early_exit_tools: &'a [&'a str],
    pause_at_cap: bool,
    max_output_tokens: Option<u32>,
    context_mw: TurnContextMiddleware,
    tool_policy: Option<ToolPolicyEnforcement>,
    deterministic_cacheable: bool,
    defer_turn_completed_to_caller: bool,
    hosted_root: Option<(
        Arc<crate::agent::tinyagents::host::OpenHumanHostBase>,
        String,
    )>,
) -> futures::future::BoxFuture<'a, Result<TinyagentsTurnOutcome>> {
    Box::pin(run_turn_via_tinyagents_body(
        run_context,
        turn_models,
        provider_id,
        model,
        history,
        tool_sets,
        allowed,
        max_iterations,
        subagent_scope,
        context_window,
        run_queue,
        early_exit_tools,
        pause_at_cap,
        max_output_tokens,
        context_mw,
        tool_policy,
        deterministic_cacheable,
        defer_turn_completed_to_caller,
        hosted_root,
    ))
}
