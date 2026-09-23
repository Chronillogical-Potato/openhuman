//! [`ToolOutputMiddleware`]: the `after_tool` ladder every tool result passes
//! through before it enters the transcript — TokenJuice compaction, payload
//! summarizer, per-tool char cap, shared byte-budget backstop, disclosure.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use tinyagents_harness::context::RunContext;
use tinyagents_harness::error::Result as TaResult;
use tinyagents_harness::events::AgentEvent;
use tinyagents_harness::middleware::{Middleware, ToolInvocationIdentity};
use tinyinference_llm::tool::ToolCall as TaToolCall;
use tinytools::{ToolPolicy as TaToolPolicy, ToolResult as TaToolResult};

use crate::agent::harness::tool_result_artifacts::{
    apply_per_result_persistence, artifact_read_target, page_artifact_read, ArtifactRead,
    ToolResultArtifactStore, TINYAGENTS_TOOL_RESULT_ARTIFACT_STORE,
};
use crate::agent::tinyagents::payload_summarizer::{
    PayloadSummarizer, SummarizeOutcome, UnavailableReason,
};
use crate::inference::tokenjuice::AgentTokenjuiceCompression;

fn estimate_output_tokens(bytes: usize) -> u64 {
    bytes.div_ceil(4) as u64
}

/// Tools whose results are self-describing JSON payloads that downstream
/// extractors and the frontend canvas parse structurally (the `type` marker
/// must survive). Compacting/summarizing them destroys the contract and
/// serves no purpose — the model doesn't benefit from a tabulated graph and
/// the payload is the turn's final output, not intermediate context.
///
/// These tools are exempt from *every* content-rewriting stage below —
/// tokenjuice compaction (steps 1+2) **and** the per-tool char cap / shared
/// byte-budget backstop (steps 3+4, see [`is_truncation_exempt`]). Both
/// `flows::ops::extract_workflow_proposal` and the frontend's
/// `parseWorkflowProposal` parse this content as a single whole-string JSON
/// document; a byte-cap truncation at a UTF-8 boundary produces invalid JSON
/// just as surely as tokenjuice tabulation strips the `"type"` marker — both
/// end in a silent `proposal: None` and a blank canvas. A ≥10-node graph
/// routinely clears the ~16 KiB shared budget, so the truncation exemption
/// matters just as much as the compaction one.
pub(crate) const COMPACTION_EXEMPT_TOOLS: &[&str] = &[
    "propose_workflow",
    "revise_workflow",
    "edit_workflow",
    "save_workflow",
    "create_workflow",
];

/// Tools whose results the model reads to derive an exact schema (e.g.
/// `primary_array_path` / `output_fields`) from a *real* sampled tool
/// response, per the B12 output-probe contract (`flows::builder_tools`).
/// TokenJuice's array-elision tabulation defeats their purpose outright — a
/// tabulated sample hides the very array shape the model is calling the tool
/// to observe, so it derives a wrong or nonexistent `split_out.path` from the
/// summary instead of the real response. They're compaction-exempt
/// ([`is_compaction_exempt`]) for that reason.
///
/// Unlike [`COMPACTION_EXEMPT_TOOLS`], their payload is intermediate context
/// the model reasons over — not the turn's final machine-parsed output — and
/// samples can be genuinely large (a full API response body). So they stay
/// subject to the per-tool char cap / shared byte-budget backstop
/// ([`is_truncation_exempt`] returns `false` for them): a truncated-but-not-
/// tabulated sample is still a usable (if partial) real response, and the
/// backstop keeps these calls from blowing the context budget.
pub(crate) const SAMPLING_TOOLS: &[&str] = &["get_tool_output_sample", "get_tool_contract"];

/// Steps 1 (tokenjuice compaction) + 2 (payload summarizer) exemption:
/// proposal tools (final-output contract, see [`COMPACTION_EXEMPT_TOOLS`])
/// plus sampling tools (tabulation would corrupt the schema they exist to
/// reveal, see [`SAMPLING_TOOLS`]).
pub(crate) fn is_compaction_exempt(name: &str) -> bool {
    COMPACTION_EXEMPT_TOOLS.contains(&name) || SAMPLING_TOOLS.contains(&name)
}

/// Steps 3 (per-tool char cap) + 4 (shared byte-budget backstop) exemption:
/// proposal tools only. Their JSON is parsed as a single whole-string
/// document downstream, so any truncation — not just tokenjuice tabulation —
/// breaks the parse. Sampling tools are deliberately *not* in this set: see
/// [`SAMPLING_TOOLS`] for why the byte cap stays in force for them.
pub(crate) fn is_truncation_exempt(name: &str) -> bool {
    COMPACTION_EXEMPT_TOOLS.contains(&name)
}

/// `after_tool`: apply the semantic payload summarizer (when configured) and
/// then the hard per-tool-result byte cap to each tool result's model-facing
/// content, before it enters the transcript. The graph analogue of the byte cap
/// + `payload_summarizer` interception the in-house `agent_tool_exec` ran.
pub(crate) struct ToolOutputMiddleware {
    /// Fallback per-tool-result byte cap for tools that don't declare their own.
    pub(crate) budget_bytes: usize,
    pub(crate) payload_summarizer: Option<Arc<dyn PayloadSummarizer>>,
    /// What the user asked for this turn, handed to the payload summarizer so
    /// it keeps the facts that matter to the task. `None` off the chat path.
    pub(crate) task_hint: Option<String>,
    pub(crate) artifact_store: Option<ToolResultArtifactStore>,
    pub(crate) tokenjuice_compaction_enabled: bool,
    pub(crate) tokenjuice_compression: AgentTokenjuiceCompression,
    /// Config resolved when the turn was constructed; avoids a disk reload from
    /// the deep `after_tool` stack.
    pub(crate) runtime_config: Option<Arc<crate::config::Config>>,
    /// SDK policy snapshot keyed by tool name. Used to honor the adapter-mapped
    /// `max_result_size_chars()` cap without re-querying the OpenHuman tool
    /// trait from `after_tool`.
    pub(crate) tool_policies: HashMap<String, TaToolPolicy>,
    /// Calls that read a persisted artifact, keyed by call id. Filled in
    /// `before_tool`, where the arguments are visible, and consumed in
    /// `after_tool`, where they are not.
    pub(crate) artifact_reads: Mutex<HashMap<String, ArtifactRead>>,
}

impl ToolOutputMiddleware {
    /// The tool's own declared cap, if any. The adapter maps OpenHuman's
    /// `max_result_size_chars()` into `ToolRuntime.max_result_bytes`; preserving
    /// char-based truncation here keeps the existing model-facing marker stable.
    pub(crate) fn tool_char_cap(&self, name: &str) -> Option<usize> {
        self.tool_policies
            .get(name)
            .and_then(|policy| policy.runtime.max_result_bytes)
    }
}

#[async_trait]
impl Middleware<(), crate::agent::tinyagents::host::OpenHumanRunContext> for ToolOutputMiddleware {
    fn name(&self) -> &str {
        "tool_output_budget"
    }

    async fn before_tool(
        &self,
        _ctx: &mut RunContext<crate::agent::tinyagents::host::OpenHumanRunContext>,
        _state: &(),
        call: &mut TaToolCall,
    ) -> TaResult<()> {
        if let Some(read) = artifact_read_target(&call.name, &call.arguments) {
            tracing::debug!(
                tool = %call.name,
                call_id = %call.id,
                path = %read.path,
                offset = read.offset,
                "[tinyagents::mw] call reads a persisted tool-result artifact"
            );
            if let Ok(mut reads) = self.artifact_reads.lock() {
                reads.insert(call.id.clone(), read);
            }
        }
        Ok(())
    }

    async fn after_tool(
        &self,
        ctx: &mut RunContext<crate::agent::tinyagents::host::OpenHumanRunContext>,
        _state: &(),
        invocation: &ToolInvocationIdentity,
        result: &mut TaToolResult,
    ) -> TaResult<()> {
        let tool_name = invocation.tool_name();
        let call_id = invocation.call_id().to_string();
        let mut content = crate::agent::tinyagents::middleware::tool_result_text(result);
        // A read of a persisted artifact is the model following the envelope's
        // `read_with` instruction. Every stage below would defeat it: the
        // summarizer re-summarizes the body the model asked to see, TokenJuice
        // compacts it, and the byte budget persists it as a *new* artifact with
        // the same bounded preview — a loop that never reaches the data (#6284).
        // Serve it verbatim, one bounded page at a time.
        let artifact_read = self
            .artifact_reads
            .lock()
            .ok()
            .and_then(|mut reads| reads.remove(&call_id));
        if let Some(read) = artifact_read {
            tracing::info!(
                tool = tool_name,
                path = %read.path,
                offset = read.offset,
                bytes = content.len(),
                "[tinyagents::mw] artifact read: skipping summarizer, compaction and re-persistence"
            );
            content = page_artifact_read(content, &read, self.budget_bytes);
            crate::agent::tinyagents::middleware::replace_tool_result_text(result, content);
            return Ok(());
        }

        // Proposal-/persistence-emitting workflow tools return a self-describing
        // `{ "type": "workflow_proposal", … }` JSON payload that `flows::ops`'
        // `extract_workflow_proposal` (and the frontend's content-based
        // recognition) parse structurally. Sampling tools (`get_tool_contract` /
        // `get_tool_output_sample`) return a real API response the model reads
        // to derive an exact array path/schema. All four stages below are
        // content-*rewriting*: tokenjuice (steps 1+2) tabulates any uniform
        // object-array of ≥3 rows over ~512 bytes into a `[json table: …]`
        // marker (stripping the `"type"` field on graphs with enough nodes, or
        // eliding the array a sample exists to reveal); the char cap and shared
        // byte-budget backstop (steps 3+4) truncate at a UTF-8 boundary, which
        // breaks the whole-string JSON parse both proposal consumers do. See
        // [`is_compaction_exempt`]/[`is_truncation_exempt`] for which stages
        // each tool family skips and why.
        let compaction_exempt = is_compaction_exempt(tool_name);
        let truncation_exempt = is_truncation_exempt(tool_name);
        if compaction_exempt {
            tracing::debug!(
                tool = tool_name,
                bytes = content.len(),
                "[tinyagents::mw] compaction-exempt: skipping tokenjuice + payload summarizer"
            );
        }
        if truncation_exempt {
            tracing::debug!(
                tool = tool_name,
                bytes = content.len(),
                "[tinyagents::mw] truncation-exempt: skipping per-tool char cap + shared byte-budget backstop"
            );
        }

        // 1. Semantic summarization (progressive disclosure) — swap the raw
        //    payload for a compressed summary when the summarizer opts in.
        //    Failures never break the tool call, but they are no longer
        //    silent: when summarization does not happen the model is told so
        //    in the payload itself. This used to be
        //    `if let Ok(Some(payload)) = …`, which discarded `Err(_)` and
        //    `Ok(None)` identically — so a failed summarization reached the
        //    model as an unannounced raw dump and it re-called the same tool.
        // Held until after the caps below rather than prefixed here. The notice
        // is ~165 chars; a tool declaring a `max_result_size_chars` smaller than
        // that had step 3 run `chars().take(cap)` straight through it, cutting
        // the reason text and the do-not-re-run sentence mid-word — so the one
        // stage that exists to stop a re-dispatch loop was removed exactly when
        // the output was most aggressively truncated. Capping the payload first
        // and prefixing afterwards also means a tool's declared cap bounds the
        // tool's own output, which is what it is a contract about, rather than
        // openhuman's annotation about it.
        let mut pending_notice: Option<&'static str> = None;
        // The byte count a summary replaced, stated in step 5 for the same
        // reason as the notice: a cap that truncates the summary must not take
        // the authoritative size with it (#6283).
        let mut summarized_from_bytes: Option<usize> = None;

        // The tool's own declared cap, read before any stage runs. It used to
        // be computed at step 3, *below* the summarizer, which meant a tool
        // declaring `max_result_size_chars(50_000)` still handed its full
        // megabyte to an LLM: the cap bounded the summary, never the
        // summarizer's input. One research turn cost 1,083,069 input tokens
        // that way, with the same page summarized three times.
        let tool_cap = self.tool_char_cap(tool_name);

        // What bounds this result. A tool that declares a cap is stating its
        // own contract and that number wins; everything else falls back to the
        // shared budget. Either way exactly one limit applies, so the two can
        // no longer double-truncate.
        let budget_bytes = tool_cap.unwrap_or(self.budget_bytes);

        // The tool's own output, kept only when it could end up persisted (step
        // 4 with a store), so the artifact stores what the tool returned rather
        // than the summarized or compacted copy the stages below produce. The
        // live artifact otherwise held 71,650 compacted bytes of a 119,796-byte
        // result and reported the smaller number as `original_bytes`. Skipped
        // when the raw body is larger than `file_read` will open: an artifact
        // nobody can read back is worse than the processed copy.
        let full_output = (!truncation_exempt
            && budget_bytes > 0
            && self.artifact_store.is_some()
            && content.len() > budget_bytes
            && content.len() as u64 <= crate::tools::FileReadTool::MAX_FILE_SIZE_BYTES)
            .then(|| content.clone());

        // A tool that declares its own cap bounds itself, and step 4 spills
        // the overflow to an artifact the model can page with `file_read`.
        // Summarizing it as well would pay a model call — at `web_fetch` sizes,
        // 20s and ~160k prompt tokens — to produce something the paging handle
        // already gives losslessly. Neither Hermes nor Codex runs a model over
        // oversized tool output; both truncate and hand back a way to read the
        // rest.
        if !compaction_exempt && tool_cap.is_none() {
            // 1. TokenJuice content-aware compaction — the cheap, deterministic
            //    step, and so the one that runs first.
            //
            //    It used to run *after* the summarizer, mirroring the legacy
            //    `agent_tool_exec` order. That made it dead weight on the only
            //    axis that costs anything: compaction never got to shrink the
            //    payload an LLM was about to read, it only ever re-compacted a
            //    summary the summarizer had already shrunk. The expensive stage
            //    paid full price for the raw bytes and the cheap stage tidied
            //    the leftovers.
            //
            //    Ordered this way, a payload TokenJuice can bring under
            //    `threshold_tokens` skips the summarizer model call entirely —
            //    `maybe_summarize_in_parent` reads `content` *after* this stage
            //    and answers `NotNeeded`. Same ladder discipline as the context
            //    ladder's "cheapest sufficient step first" (#6014).
            //
            //    This is only sound because TokenJuice's transforms are
            //    representation changes an LLM can still read (tabulating a
            //    uniform object-array into a `[json table: …]` marker), not
            //    erasure. The context ladder's own ordering bug is the
            //    counter-example to respect: microcompact *blanks* tool bodies
            //    to `CLEARED_PLACEHOLDER`, so running it before summarization
            //    asked the summarizer for "key results" it could no longer see.
            //    Any future TokenJuice profile that drops content outright
            //    rather than re-encoding it belongs behind the summarizer
            //    again.
            //
            //    Compaction is off by default (`[context].compaction_enabled`
            //    is `false` and the router lives behind the TinyBus module
            //    boundary), so on a default install
            //    `compact_output_with_config` returns `content` untouched and
            //    this stage changes nothing. The ordering matters for installs
            //    that turn it on.
            let before_tokenjuice_bytes = content.len();
            let compacted = crate::inference::tokenjuice::compact_output_with_config(
                std::mem::take(&mut content),
                tool_name,
                self.tokenjuice_compaction_enabled,
                self.tokenjuice_compression,
                self.runtime_config.as_ref(),
            )
            .await;
            content = compacted;
            let after_tokenjuice_bytes = content.len();
            if after_tokenjuice_bytes < before_tokenjuice_bytes {
                ctx.emit(AgentEvent::Compressed {
                    from_tokens: estimate_output_tokens(before_tokenjuice_bytes),
                    to_tokens: estimate_output_tokens(after_tokenjuice_bytes),
                });
            }

            // 2. Semantic summarization, on whatever step 1 left behind.
            if let Some(ps) = &self.payload_summarizer {
                match ps
                    .maybe_summarize_in_parent(ctx, tool_name, self.task_hint.as_deref(), &content)
                    .await
                {
                    Ok(SummarizeOutcome::Summarized(payload)) => {
                        tracing::info!(
                            tool = tool_name,
                            from_bytes = payload.original_bytes,
                            to_bytes = payload.summary_bytes,
                            "[tinyagents::mw] payload_summarizer compressed tool output"
                        );
                        ctx.emit(AgentEvent::Compressed {
                            from_tokens: estimate_output_tokens(payload.original_bytes),
                            to_tokens: estimate_output_tokens(payload.summary_bytes),
                        });
                        summarized_from_bytes = Some(payload.original_bytes);
                        content = payload.summary;
                    }
                    // The payload was fine as it was. Say nothing: a notice on
                    // every small tool result would be pure noise.
                    Ok(SummarizeOutcome::NotNeeded) => {}
                    Ok(SummarizeOutcome::Unavailable(reason)) => {
                        tracing::warn!(
                            tool = tool_name,
                            bytes = content.len(),
                            ?reason,
                            "[tinyagents::mw] payload_summarizer unavailable; disclosing raw output"
                        );
                        pending_notice = Some(reason.notice());
                    }
                    // Reserved for fatal misconfiguration. Previously
                    // indistinguishable from "nothing to do"; the model is now
                    // told the output is raw for the same reason as above.
                    Err(error) => {
                        tracing::warn!(
                            tool = tool_name,
                            bytes = content.len(),
                            error = %error,
                            "[tinyagents::mw] payload_summarizer errored; disclosing raw output"
                        );
                        pending_notice = Some(UnavailableReason::Failed.notice());
                    }
                }
            }
        }

        // 3. One bound, one place. Whether the limit came from the tool's own
        //    `max_result_size_chars` or from the shared budget, an oversized
        //    result takes the same route: spill the full body to an artifact,
        //    hand back a preview plus the `file_read` call that pages the rest,
        //    and fall back to an inline marker when no store is configured.
        //
        //    Declaring a cap used to *disable* this — `tool_cap.is_none()`
        //    gated the persistence path — so the tools most in need of a
        //    recovery handle were the ones denied it. `web_fetch` discarded
        //    everything past 50k chars with no way to get it back, while
        //    `file_read`, which declares no cap, got full byte-offset paging.
        //    This is the affordance Hermes' `web_extract` footer provides and
        //    the one `web_fetch`'s own doc comment already recommended.
        //
        //    This is a per-result cap only — `apply_per_result_persistence`
        //    takes a single `content: String` and a fixed budget, with no
        //    shared/global accumulator across tool calls (the aggregate-spill
        //    variant, `spill_aggregate_tool_results`, is a separate legacy code
        //    path not wired into this middleware) — so exempting a tool's own
        //    contribution here cannot perturb any other tool's accounting.
        if !truncation_exempt && budget_bytes > 0 {
            let (capped, outcome) = apply_per_result_persistence(
                std::mem::take(&mut content),
                full_output,
                self.artifact_store.as_ref(),
                tool_name,
                Some(&call_id),
                budget_bytes,
            )
            .await;
            if outcome.persisted {
                tracing::info!(
                    tool = tool_name,
                    from_bytes = outcome.original_bytes,
                    to_bytes = outcome.final_bytes,
                    "[tinyagents::mw] tool_result_artifact persisted oversized output"
                );
                if let Some(path) = outcome.artifact_path.as_deref() {
                    if let Some(store) = ctx.stores.get(TINYAGENTS_TOOL_RESULT_ARTIFACT_STORE) {
                        let key = call_id.clone();
                        let mut fields = serde_json::Map::new();
                        fields.insert("tool".to_string(), tool_name.into());
                        fields.insert("call_id".to_string(), call_id.clone().into());
                        fields.insert("artifact_path".to_string(), path.to_string().into());
                        fields.insert(
                            "original_bytes".to_string(),
                            serde_json::Value::from(outcome.original_bytes as u64),
                        );
                        fields.insert(
                            "preview_bytes".to_string(),
                            serde_json::Value::from(outcome.final_bytes as u64),
                        );
                        let index_result: tinyagents_harness::Result<()> =
                            store.put("tool_results", &key, fields.into()).await;
                        if let Err(err) = index_result {
                            tracing::warn!(
                                tool = tool_name,
                                call_id = %call_id,
                                error = %err,
                                "[tinyagents::mw] failed to index tool_result_artifact"
                            );
                        } else {
                            tracing::debug!(
                                tool = tool_name,
                                call_id = %call_id,
                                artifact_path = %path,
                                "[tinyagents::mw] indexed tool_result_artifact in run store"
                            );
                        }
                    }
                }
            } else if outcome.original_bytes != outcome.final_bytes {
                tracing::debug!(
                    tool = tool_name,
                    from_bytes = outcome.original_bytes,
                    to_bytes = outcome.final_bytes,
                    "[tinyagents::mw] tool_result_budget truncated tool output"
                );
            }
            content = capped;
        }

        // 5. The disclosure, last, so no cap above can eat it. The model has to
        //    be able to read *why* the payload is raw and that re-running will
        //    not summarize it — a half-truncated notice is worse than none,
        //    because it still looks like tool output.
        if let Some(notice) = pending_notice {
            content = format!("{notice}\n\n{content}");
        }
        if let Some(bytes) = summarized_from_bytes {
            content = format!(
                "[openhuman: summary of {bytes} bytes of tool output, complete]\n\n{content}"
            );
        }

        crate::agent::tinyagents::middleware::replace_tool_result_text(result, content);

        Ok(())
    }
}

#[cfg(test)]
#[path = "tool_output_tests.rs"]
mod tests;
