//! Bound direct web research so a run that keeps finding more leads still answers.
//!
//! Only web access is bounded. Once the budget is spent the web tools leave the
//! request and the run carries on with the rest; the turn is forced to answer
//! only when no other tool remains (#6959).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use async_trait::async_trait;
use tinyagents_harness::context::RunContext;
use tinyagents_harness::error::Result as TaResult;
use tinyagents_harness::middleware::{Middleware, ToolInvocationIdentity};
use tinyinference_llm::message::Message;
use tinyinference_llm::model::{ModelRequest, ToolChoice};
use tinytools::ToolResult;

use crate::agent::tinyagents::host::OpenHumanRunContext;

/// Direct web reads allowed before the web tools are withdrawn.
/// This bounds the master agent's exploratory search while leaving room to read
/// primary sources; broad research belongs in one deep `web_answer_tool` call or
/// a batched `web_contents_tool` read, each of which counts once.
pub(super) const DIRECT_WEB_READ_LIMIT: usize = 8;

/// The instruction appended when the budget is spent and no non-web tool
/// remains, so every tool is withdrawn. It says outright that tools are gone
/// and a call will not run: told only to "answer", a native model with a
/// transcript full of tool calls and no tool channel writes its next call as
/// plain-text markup (DeepSeek V4's
/// `<｜DSML｜invoke …>`), which then stood as the turn's answer. Replaying a
/// bench request that hit this budget, the old wording leaked a call 6 times in
/// 8 and this wording 0 times in 8. The harness also withholds and re-prompts
/// any call that still arrives (tinyagents `TextRecovery::withholding`).
pub(crate) const RESEARCH_CLOSE_INSTRUCTION: &str = "The direct web research budget for this turn is exhausted, and tools are no longer available for this reply: any tool call you write now will not run. Answer the user's latest request now in plain text, using only the results already available. State any remaining uncertainty. Do not search again, repeat a page fetch, or merely describe what you plan to read.";

/// The instruction appended when the budget is spent but the run still has
/// non-web tools: only web access is exhausted, not the task (#6959).
pub(crate) const WEB_BUDGET_EXHAUSTED_INSTRUCTION: &str = "The direct web research budget for this turn is exhausted, so the web tools have been removed. Continue the task with your remaining tools, using the web results already available. Do not try to search or fetch pages again.";

/// Consecutive failed web calls after which web access looks blocked.
const BLOCKED_AFTER_FAILURES: usize = 2;

/// The one-time note sent after [`BLOCKED_AFTER_FAILURES`] failed web calls in a
/// row (a blocked network, an offline sandbox).
pub(crate) const WEB_BLOCKED_NOTE: &str = "Web access looks blocked: the last web calls failed. Do not keep retrying web tools; continue the task with your other tools and the information already available.";

/// The direct web-read tools this budget governs: the `web_search` family
/// (`tools::user_filter`) plus `web_fetch`.
fn is_web_research_tool(name: &str) -> bool {
    matches!(
        name,
        "web_search_tool" | "web_answer_tool" | "web_contents_tool" | "web_fetch"
    )
}

#[derive(Default)]
pub(crate) struct ResearchBudgetMiddleware {
    /// Successful web reads. A failed call read nothing and is not counted.
    completed_reads: AtomicUsize,
    /// Failed web calls since the last successful one.
    consecutive_failures: AtomicUsize,
    /// Set once the blocked note is due; cleared when it is sent.
    blocked_note_pending: AtomicBool,
    /// Whether the blocked note was already queued this run.
    blocked_note_sent: AtomicBool,
}

impl ResearchBudgetMiddleware {
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl Middleware<(), OpenHumanRunContext> for ResearchBudgetMiddleware {
    fn name(&self) -> &str {
        "research_budget"
    }

    async fn after_tool(
        &self,
        _ctx: &mut RunContext<OpenHumanRunContext>,
        _state: &(),
        invocation: &ToolInvocationIdentity,
        result: &mut ToolResult,
    ) -> TaResult<()> {
        let tool = invocation.tool_name();
        if !is_web_research_tool(tool) {
            return Ok(());
        }
        if !result.is_error {
            self.consecutive_failures.store(0, Ordering::Relaxed);
            self.completed_reads.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }
        let failures = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
        tracing::debug!(
            tool,
            consecutive_failures = failures,
            "[tinyagents::mw] research_budget: failed web call not counted against the budget"
        );
        if failures >= BLOCKED_AFTER_FAILURES
            && !self.blocked_note_sent.swap(true, Ordering::Relaxed)
        {
            tracing::info!(
                consecutive_failures = failures,
                "[tinyagents::mw] research_budget: web access looks blocked; queueing one-time note"
            );
            self.blocked_note_pending.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    async fn before_model(
        &self,
        _ctx: &mut RunContext<OpenHumanRunContext>,
        _state: &(),
        request: &mut ModelRequest,
    ) -> TaResult<()> {
        let reads = self.completed_reads.load(Ordering::Relaxed);
        if reads < DIRECT_WEB_READ_LIMIT {
            if self.blocked_note_pending.swap(false, Ordering::Relaxed) {
                request
                    .messages
                    .push(Message::user(WEB_BLOCKED_NOTE.to_string()));
            }
            return Ok(());
        }
        // The budget note supersedes a pending blocked note: the web tools are
        // gone either way.
        self.blocked_note_pending.store(false, Ordering::Relaxed);
        request
            .tools
            .retain(|tool| !is_web_research_tool(&tool.name));
        if request.tools.is_empty() {
            tracing::info!(
                web_reads = reads,
                "[tinyagents::mw] research_budget: budget reached and no non-web tool remains; concluding turn"
            );
            request.tool_choice = ToolChoice::None;
            request
                .messages
                .push(Message::user(RESEARCH_CLOSE_INSTRUCTION.to_string()));
            return Ok(());
        }
        tracing::info!(
            web_reads = reads,
            remaining_tools = request.tools.len(),
            "[tinyagents::mw] research_budget: budget reached; withdrawing web tools only"
        );
        if matches!(&request.tool_choice, ToolChoice::Tool(name) if is_web_research_tool(name)) {
            request.tool_choice = ToolChoice::Auto;
        }
        request
            .messages
            .push(Message::user(WEB_BUDGET_EXHAUSTED_INSTRUCTION.to_string()));
        Ok(())
    }
}
