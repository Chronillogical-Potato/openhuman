//! Test-only thin harness drive, kept separate from the production runner.

use super::*;
use crate::agent::tinyagents::model::{ProfileOverrideModel, TurnChatModel};
use crate::agent::tinyagents::turn_policy::run_policy_for;

pub(crate) async fn run_turn_via_tinyagents(
    chat_model: TurnChatModel,
    model: &str,
    temperature: f64,
    history: Vec<ChatMessage>,
    resolved_tools: Vec<Arc<dyn tinytools::Tool>>,
    max_iterations: usize,
) -> Result<TinyagentsTurnOutcome> {
    let max_iterations = effective_max_iterations(max_iterations);
    let mut harness: tinyagents_harness::runtime::AgentHarness<()> =
        tinyagents_harness::runtime::AgentHarness::new();
    harness.with_policy(run_policy_for(max_iterations, false));
    let profile = chat_model.profile().cloned().unwrap_or_default();
    let chat_model: TurnChatModel = Arc::new(
        ProfileOverrideModel::new(chat_model, profile)
            .with_request_model(model)
            .with_request_temperature(temperature),
    );
    let error_slot = Arc::new(std::sync::Mutex::new(None));
    harness
        .register_model(model, chat_model)
        .set_default_model(model);
    let tool_count = resolved_tools.len();
    for tool in resolved_tools {
        harness.register_tool(tool);
    }
    let config = crate::agent::tinyagents::host::run_context::fresh_root_run_config("agent-turn")
        .with_max_model_calls(max_iterations)
        .with_max_tool_calls(max_iterations.saturating_mul(8).max(8))
        .with_max_depth(MAX_SPAWN_DEPTH)
        .with_tag("openhuman")
        .with_tag("scope:root")
        .with_tag("unobserved");
    tracing::info!(
        model,
        max_iterations,
        tools = tool_count,
        "[tinyagents] routing agent turn through tinyagents harness"
    );
    let input = crate::agent::message_convert::history_to_messages(&history);
    let request_base_len = input.len();
    let run = match Box::pin(harness.invoke(&(), (), config, input)).await {
        Ok(run) => run,
        Err(error) => {
            if let Some(original) = error_slot
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
            {
                return Err(original);
            }
            return Err(anyhow::anyhow!("tinyagents harness run failed: {error}"));
        }
    };
    let text = run.text().unwrap_or_default();
    let out_history = crate::agent::message_convert::messages_to_history(&run.messages);
    let conversation = crate::agent::message_convert::messages_to_conversation(
        crate::agent::message_convert::messages_since_request(&run.messages, request_base_len),
    );
    tracing::debug!(
        request_base_len,
        transcript_len = run.messages.len(),
        persisted_messages = run.messages.len().saturating_sub(request_base_len),
        "[tinyagents] persisting post-request transcript (thin path; steer-safe boundary)"
    );
    Ok(TinyagentsTurnOutcome {
        text,
        resolved_route: None,
        history: out_history,
        conversation,
        model_calls: run.model_calls,
        tool_calls: run.tool_calls,
        input_tokens: run.usage.usage.input_tokens,
        output_tokens: run.usage.usage.output_tokens,
        cached_input_tokens: run.usage.usage.cache_read_tokens,
        charged_amount_usd: crate::platform::cost::catalog::estimate_cost_usd(
            model,
            run.usage.usage.input_tokens,
            run.usage.usage.output_tokens,
            run.usage.usage.cache_read_tokens,
        ),
        early_exit_tool: None,
        hit_cap: false,
        wrap_up_injected: false,
        breaker_halt: None,
        tool_outcomes: Vec::new(),
    })
}
