//! [`FinalCallWrapUpMiddleware`]: turn the last permitted model call of a
//! capped turn into the turn's conclusion, inside the loop (issue #6014).

use std::sync::Arc;

use async_trait::async_trait;

use tinyagents_harness::context::RunContext;
use tinyagents_harness::error::Result as TaResult;
use tinyagents_harness::middleware::Middleware;
use tinyinference_llm::message::{ContentBlock, Message as TaMessage};
use tinyinference_llm::model::ModelRequest;

use crate::agent::context::CLEARED_PLACEHOLDER;

use super::message_trim::{estimate_message_tokens, estimate_text_tokens};

/// Turns the **last permitted model call of a capped turn** into the turn's
/// conclusion, in the loop, instead of leaving the answer to an extra call
/// made after the loop has already exited.
///
/// # Why the loop and not afterwards
///
/// A capped turn used to end like this: the loop exits on the model-call cap,
/// and `OpenHumanSessionHost::summarize_turn_wrapup` then dispatches a second, out-of-band
/// request straight at the `ChatModel` asking for a checkpoint. Being outside
/// the harness, that request ran with **none** of the loop's context
/// management — no microcompact, no compression, no trim — while being built
/// from the largest transcript the turn would ever hold (every tool result a
/// full iteration budget produced). It was therefore the likeliest call of the
/// whole turn to overflow the window, and each of its failure paths returns
/// `("", None)` silently, so the answer degraded to a deterministic digest of
/// tool names exactly when the turn had the most to report. It also bypassed
/// usage accounting (folded back by hand at the call site) and the progress
/// bridge (re-implemented there as buffer-then-validate-then-forward).
///
/// Doing it here removes all of that rather than compensating for it: the
/// concluding call is an ordinary loop iteration, so it inherits the entire
/// middleware stack, its usage rides `UsageCarryMiddleware` like any other
/// call, and its text streams through the normal event bridge. It is also one
/// provider call cheaper — the wrap-up was an extra call *past* the cap the
/// operator configured.
///
/// # What "last permitted call" means, exactly
///
/// The loop records the model call **before** it builds the request
/// (`agent_loop::run_loop`), so by the time `before_model` runs for the Nth
/// call of an N-call budget, `remaining_model_calls()` is already `0`. That is
/// the trigger, and it needs no new plumbing or counter of its own.
///
/// The trade this makes is explicit: a 25-call budget becomes 24 tool rounds
/// plus a conclusion, rather than 25 tool rounds plus a 26th call nobody
/// budgeted for.
///
/// # Why the tools are cleared rather than merely discouraged
///
/// The instruction alone is a request the model may ignore — the out-of-band
/// wrap-up had to re-parse its own response through the dispatcher to catch a
/// model that emitted a tool call anyway. Removing the schemas from the
/// request makes it structural instead: there is nothing to call. `tool_choice`
/// is reset alongside them because a `Required` choice with an empty tool array
/// is a provider 400.
/// The tools left on the belt for the **penultimate** call of a capped turn
/// (see [`FinalCallWrapUpMiddleware::reserve_final_write`]).
///
/// The membership rule is "can only emit, never gather". Both of these write a
/// file the caller already knows the contents of, so neither can be spent
/// discovering something the turn then has no room to report — which is what
/// makes reserving the call for them a safe trade rather than a gamble.
///
/// `file_write` is the only create-capable file tool (it resolves through
/// `validate_parent_path` rather than `validate_path`); `apply_patch` gained a
/// create mode in #6548 and is the one an agent editing an existing artifact
/// reaches for. `shell` is deliberately absent even though it can redirect into
/// a file: it can equally run a crawler, so keeping it would leave the belt
/// effectively unnarrowed.
pub(crate) const DELIVERABLE_TOOLS: &[&str] = &["file_write", "apply_patch"];

/// Whether a tool is one the penultimate call keeps.
fn is_deliverable_tool(name: &str) -> bool {
    DELIVERABLE_TOOLS.contains(&name)
}

pub(crate) struct FinalCallWrapUpMiddleware {
    /// The synthetic user turn appended on the final call.
    instruction: &'static str,
    /// The synthetic user turn appended on the call before it, when the belt is
    /// narrowed to [`DELIVERABLE_TOOLS`] instead of cleared.
    final_write_instruction: &'static str,
    /// Every tool call's captured outcome, so the concluding call can be given
    /// back the results microcompact blanked (see `before_model`).
    outcomes: crate::agent::tinyagents::ToolOutcomeSink,
    /// The input-token allowance the trim downstream enforces, so restoration
    /// can stay under it rather than provoking an eviction. `0` disables the
    /// bound (a model advertising no context window).
    input_budget: u64,
    /// Set when the injection fires, so the caller can report the turn as
    /// capped. Necessary because this turn now ends *naturally* — the model
    /// returns text and requests no tools, which is the loop's ordinary
    /// terminal condition — so the old `final_response.is_none()` tell no
    /// longer distinguishes a capped turn from a finished one.
    fired: Arc<std::sync::atomic::AtomicBool>,
}

impl FinalCallWrapUpMiddleware {
    pub(crate) fn new(
        instruction: &'static str,
        final_write_instruction: &'static str,
        outcomes: crate::agent::tinyagents::ToolOutcomeSink,
        input_budget: u64,
    ) -> Self {
        Self {
            instruction,
            final_write_instruction,
            outcomes,
            input_budget,
            fired: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// The shared flag, for the run loop to read after the drive future returns.
    pub(crate) fn fired(&self) -> Arc<std::sync::atomic::AtomicBool> {
        self.fired.clone()
    }

    /// Give a concluding-or-persisting call back the tool results microcompact
    /// blanked, newest-first and only while the request still fits.
    ///
    /// Shared by both of this middleware's calls, because both need the same
    /// thing for the same reason: the content the turn gathered. The final call
    /// needs it to *report* findings, and the penultimate one needs it to
    /// *write them into a file* — and a turn asked to produce an artifact from
    /// nineteen rounds of "[Old tool result content cleared]" produces the same
    /// empty-handed result from either direction.
    ///
    /// `instruction` is the text the caller will append afterwards; it is
    /// seeded into the token accounting here rather than counted later, because
    /// restoration fills the budget to its boundary and an unaccounted fixed
    /// addition after it is exactly the overshoot the budget exists to prevent
    /// (CodeRabbit on #6068).
    fn restore_cleared_outcomes(&self, request: &mut ModelRequest, instruction: &str) -> usize {
        let budget = self.input_budget;
        // Seeded with the instruction this middleware appends unconditionally
        // below, not just with what the request already holds (CodeRabbit on
        // #6068). Restoration fills the budget to its boundary, so an
        // unaccounted fixed addition after it is exactly the overshoot the
        // budget exists to prevent.
        let mut used: u64 = request
            .messages
            .iter()
            .map(estimate_message_tokens)
            .sum::<u64>()
            .saturating_add(estimate_text_tokens(instruction));
        let restored = match self.outcomes.lock() {
            Ok(outcomes) => {
                let mut restored = 0usize;
                let mut skipped = 0usize;
                for message in request.messages.iter_mut().rev() {
                    let TaMessage::Tool(tool) = message else {
                        continue;
                    };
                    if tool
                        .content
                        .iter()
                        .any(|block| !matches!(block, ContentBlock::Text(_)))
                    {
                        continue;
                    }
                    let body: String = tool
                        .content
                        .iter()
                        .filter_map(|block| match block {
                            ContentBlock::Text(text) => Some(text.as_str()),
                            _ => None,
                        })
                        .collect();
                    if body.trim() != CLEARED_PLACEHOLDER {
                        continue;
                    }
                    let Some(outcome) = captured_outcome_for(&outcomes, &tool.tool_call_id) else {
                        continue;
                    };
                    if outcome.trim().is_empty() {
                        continue;
                    }
                    // What restoring this body would add, against what the
                    // placeholder already costs.
                    let added = estimate_text_tokens(&outcome)
                        .saturating_sub(estimate_text_tokens(CLEARED_PLACEHOLDER));
                    if budget > 0 && used.saturating_add(added) > budget {
                        // Everything older is at least as likely to overflow, but
                        // keep counting so the log reports the true shortfall
                        // rather than stopping at the first one that did not fit.
                        skipped += 1;
                        continue;
                    }
                    used = used.saturating_add(added);
                    tool.content = vec![ContentBlock::Text(outcome)];
                    restored += 1;
                }
                if skipped > 0 {
                    tracing::info!(
                        skipped,
                        restored,
                        budget,
                        used,
                        "[tinyagents::mw] left some cleared tool results cleared: restoring them \
                         would have pushed the concluding call past its input budget, and an \
                         eviction there costs whole messages rather than one body"
                    );
                }
                restored
            }
            Err(_) => {
                tracing::warn!(
                    "[tinyagents::mw] tool-outcome sink poisoned; concluding without restoring \
                     cleared tool results"
                );
                0
            }
        };
        if restored > 0 {
            tracing::info!(
                restored,
                "[tinyagents::mw] restored cleared tool results for the concluding call"
            );
        }
        restored
    }
}

impl FinalCallWrapUpMiddleware {
    /// The call *before* the conclusion: narrow the belt to
    /// [`DELIVERABLE_TOOLS`] so a turn that owes a file can still write it.
    ///
    /// Clearing the belt one call later makes the conclusion structural, which
    /// is right — but for a turn whose product is an artifact rather than
    /// prose it makes *failure* structural too. See
    /// [`FINAL_WRITE_INSTRUCTION`](crate::agent::session_host::turn_checkpoint::FINAL_WRITE_INSTRUCTION)
    /// for the case that motivated this and the trade it accepts.
    ///
    /// Returns `true` when the narrowing fired, so the caller can skip the
    /// instruction otherwise.
    fn reserve_final_write(
        &self,
        ctx: &RunContext<crate::agent::tinyagents::host::OpenHumanRunContext>,
        request: &mut ModelRequest,
    ) -> bool {
        // Below three, reserving would eat the turn rather than shape its end:
        // a two-call budget would be one write-only call plus the conclusion,
        // leaving no round in which anything could be gathered to write.
        if ctx.limits.limits().max_model_calls <= 2 {
            return false;
        }
        // Nothing to reserve the call *for*. A read-only or delegating agent
        // has no writer on its belt, and telling it "the only tools left are
        // the ones that write files" would be false — so leave the call as an
        // ordinary one and let the conclusion handle the cap.
        if !request.tools.iter().any(|t| is_deliverable_tool(&t.name)) {
            return false;
        }
        let before = request.tools.len();
        request.tools.retain(|t| is_deliverable_tool(&t.name));
        // `Auto`, never `Required`: a turn that has already written its file,
        // or was only ever asked for an answer, must be free to spend this call
        // on text instead. Forcing a call here would make it invent a write.
        request.tool_choice = tinyinference_llm::model::ToolChoice::Auto;
        tracing::info!(
            model_calls = ctx.limits.model_calls(),
            max_model_calls = ctx.limits.limits().max_model_calls,
            tools_withdrawn = before.saturating_sub(request.tools.len()),
            tools_kept = request.tools.len(),
            "[tinyagents::mw] penultimate model call — narrowing the belt to the tools that can \
             persist a deliverable"
        );
        // The same restoration the conclusion gets, and for a sharper reason:
        // this call is being asked to write the findings into a file, so it
        // needs to be able to read them.
        self.restore_cleared_outcomes(request, self.final_write_instruction);
        request
            .messages
            .push(TaMessage::user(self.final_write_instruction.to_string()));
        true
    }
}

#[async_trait]
impl Middleware<(), crate::agent::tinyagents::host::OpenHumanRunContext>
    for FinalCallWrapUpMiddleware
{
    fn name(&self) -> &str {
        "final_call_wrap_up"
    }

    async fn before_model(
        &self,
        ctx: &mut RunContext<crate::agent::tinyagents::host::OpenHumanRunContext>,
        _state: &(),
        request: &mut ModelRequest,
    ) -> TaResult<()> {
        let remaining = ctx.limits.remaining_model_calls();
        if remaining > 1 {
            return Ok(());
        }
        // A budget of one call would make the very first call the concluding
        // one, so the turn could never run a tool at all. That is a
        // misconfiguration rather than a cap being reached, and silently
        // answering it with a "you have run out of tool calls" instruction
        // would misreport it — leave such a run alone.
        if ctx.limits.limits().max_model_calls <= 1 {
            return Ok(());
        }
        // One call before the conclusion: keep the writers rather than clear
        // the belt, so a turn whose deliverable is a file can still produce it.
        if remaining == 1 {
            self.reserve_final_write(ctx, request);
            return Ok(());
        }
        tracing::info!(
            model_calls = ctx.limits.model_calls(),
            max_model_calls = ctx.limits.limits().max_model_calls,
            tools_withdrawn = request.tools.len(),
            "[tinyagents::mw] final permitted model call — withdrawing tools and asking for the \
             turn's conclusion"
        );
        request.tools.clear();
        request.tool_choice = tinyinference_llm::model::ToolChoice::None;
        // Give the concluding call back the results microcompact blanked.
        //
        // `MicrocompactMiddleware` replaces every tool-result body past the
        // most recent `keep_recent` (5, by default) with `CLEARED_PLACEHOLDER`,
        // and — constructed without a token budget — it does so on every call,
        // not only under context pressure. That is right for an intermediate
        // call, which needs recent context to choose the next tool and nothing
        // more. It is exactly wrong for this one: a turn that spent 24 rounds
        // gathering would be asked to report its findings with 19 rounds of
        // them replaced by "[Old tool result content cleared]", which is the
        // same empty-handed answer this whole mechanism exists to prevent,
        // arrived at from the other direction.
        //
        // Restored from the captured outcomes rather than by exempting the
        // turn from microcompact, because the blanking has already happened by
        // the time this runs (registration order: microcompact is installed by
        // `context_mw.install`, this middleware immediately after it) and
        // because the sink is the honest source — it holds each result as it
        // entered the transcript, after the per-result byte cap.
        //
        // Only a body that IS the placeholder is replaced, so a result the
        // model legitimately saw in full is never rewritten.
        //
        // This deliberately runs BEFORE the compression and trim middlewares,
        // which are installed after it: restoring can make the request large,
        // and those two are what bound it. The resulting degradation ladder is
        // the one this call wants — everything when it fits, an LLM summary of
        // the older slice when it does not, and oldest-first eviction only in
        // extremis. What it never does again is silently blank the middle.
        // Restore newest-first, and only while the request still fits.
        //
        // CodeRabbit on #6068: restoration runs AFTER `ContextCompressionMiddleware`
        // and before `ImageAwareMessageTrimMiddleware`, so an unbounded restore can
        // push the request over the window and the trim then evicts whole
        // messages — which is strictly more destructive than the blanking being
        // undone, and can discard the very results just restored.
        //
        // The review suggested a compression phase after restoration. That is a
        // second summarizer model call on the one call already about to produce
        // the conclusion, and it is avoidable: the overflow is preventable
        // rather than repairable. Restoring under the same budget the trim
        // enforces means the trim never has cause to fire, so nothing is
        // evicted and nothing needs re-summarising.
        //
        // Newest-first because recency is relevance here: the last rounds'
        // results are the ones the model has not seen (the cap is checked
        // before the request is built), and the earliest ones are most likely
        // already reflected in the compression summary above.
        self.restore_cleared_outcomes(request, self.instruction);
        request
            .messages
            .push(TaMessage::user(self.instruction.to_string()));
        self.fired.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

/// The captured content for one tool call id, if the sink holds it.
///
/// A free function so the borrow of the locked sink stays scoped to the lookup
/// rather than being held across the mutation of `request.messages`. Named
/// without a `self_` prefix (tinysweeper on #6068): it takes no receiver, and
/// the prefix read as a method on something.
fn captured_outcome_for(
    outcomes: &[crate::agent::tinyagents::ToolCallOutcome],
    call_id: &str,
) -> Option<String> {
    outcomes
        .iter()
        .find(|outcome| outcome.call_id == call_id)
        .map(|outcome| outcome.content.clone())
}
