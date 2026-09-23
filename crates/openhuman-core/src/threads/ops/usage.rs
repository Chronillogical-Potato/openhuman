//! Aggregated token/cost usage for a thread, re-audited at current pricing.

use super::support::{counts, envelope, workspace_dir};
use crate::memory::ApiEnvelope;
use crate::rpc::RpcOutcome;
use std::collections::BTreeMap;
use std::path::Path;
use tinyagents_session::transcript::{
    find_root_transcripts_for_thread, read_transcript, SessionTranscript,
};

/// Request for [`token_usage`]: the thread whose persisted usage to total.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ThreadTokenUsageRequest {
    pub thread_id: String,
}

/// Aggregated token/cost usage for one thread, read back from its persisted
/// session transcripts. Seeds the UI footer when the user selects a thread so
/// the totals reflect prior turns instead of starting at zero.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ThreadTokenUsageResponse {
    pub thread_id: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub cost_usd: f64,
    pub turn_count: usize,
    /// Tokens of the most recent turn — numerator for the context-window gauge.
    /// The orchestrator's own, excluding sub-agents: each child runs in its own
    /// context window, so folding them in let the gauge exceed 100% (#4271).
    pub last_turn_input_tokens: u64,
    pub last_turn_output_tokens: u64,
    /// Context window (tokens) inferred from the last model; `0` when unknown.
    pub context_window: u64,
    pub model: Option<String>,
    pub updated: Option<String>,
    /// `false` when the thread has no persisted spend yet (all zeros). The UI
    /// uses this to decide whether to seed its live bucket at all, so a thread
    /// whose transcripts exist but recorded nothing must report `false` — the
    /// alternative overwrites a live in-progress bucket with zeros.
    pub has_usage: bool,
    /// Per-archetype sub-agent spend (re-audited at current pricing). The
    /// top-level totals already include this; it's broken out for the UI's
    /// per-agent footer rows.
    pub subagents: Vec<SubagentUsageDto>,
}

/// One sub-agent archetype's contribution within a thread.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SubagentUsageDto {
    pub agent_id: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    pub runs: usize,
}

/// One transcript's own spend, summed from the per-turn `turn_usage` records
/// the codec attaches to its assistant rows.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct TranscriptSpend {
    pub(super) input_tokens: u64,
    pub(super) output_tokens: u64,
    pub(super) cached_input_tokens: u64,
    pub(super) cost_usd: f64,
    /// Turns that recorded usage. The codec emits one record per durable
    /// append, and omits an all-zero one, so this is "turns that spent".
    pub(super) turns: usize,
    /// The newest record's model and window, for the caller's last-turn view.
    pub(super) last_input_tokens: u64,
    pub(super) last_output_tokens: u64,
    pub(super) model: Option<String>,
    pub(super) context_window: u64,
}

/// Total one transcript file's own recorded spend.
///
/// The `_meta` header carries denormalised rollups for the same figures, but
/// nothing has written them on the root path since the TinyAgents runtime
/// cutover (`33566d382`) — every root transcript since reads `input_tokens: 0`
/// while its per-turn records hold the real numbers (#6460). The per-turn
/// records are the authoritative copy and the only one that is written on every
/// path, so this reads those and ignores the header.
///
/// `read_transcript` is the reader that applies compaction records and drops
/// interrupted partials, so a compacted or interrupted session is summed over
/// its logical message set rather than its raw append log.
pub(super) fn transcript_spend(transcript: &SessionTranscript) -> TranscriptSpend {
    let mut spend = TranscriptSpend::default();
    for message in &transcript.messages {
        let Some(usage) = message.turn_usage.as_ref() else {
            continue;
        };
        spend.input_tokens = spend.input_tokens.saturating_add(usage.usage.input);
        spend.output_tokens = spend.output_tokens.saturating_add(usage.usage.output);
        spend.cached_input_tokens = spend
            .cached_input_tokens
            .saturating_add(usage.usage.cached_input);
        spend.cost_usd += usage.usage.cost_usd;
        spend.turns += 1;
        spend.last_input_tokens = usage.usage.input;
        spend.last_output_tokens = usage.usage.output;
        if usage.usage.context_window > 0 {
            spend.context_window = usage.usage.context_window;
        }
        if !usage.model.is_empty() {
            spend.model = Some(usage.model.clone());
        }
    }
    // A transcript whose model never landed on a usage record still has one in
    // its header; prefer the record, fall back to the header.
    if spend.model.is_none() {
        spend.model = transcript.meta.model.clone();
    }
    spend
}

/// Read one transcript, logging and skipping an unreadable file rather than
/// failing the whole thread's aggregate for it.
fn read(path: &Path) -> Option<SessionTranscript> {
    match read_transcript(path) {
        Ok(transcript) => Some(transcript),
        Err(err) => {
            log::warn!(
                "[threads:usage] skipping unreadable transcript {}: {err}",
                path.display()
            );
            None
        }
    }
}

/// Every descendant transcript of `root`, at any delegation depth.
///
/// Sub-agent transcripts are named `{root_stem}__{child}` (and a grandchild
/// chains another `__`), which is the *only* durable record of the parent→child
/// relation. Selecting them by `_meta.thread_id` instead — as the session
/// crate's own summary helper does — drops the majority of them: a delegation
/// gets its own worker thread, and `inherited_thread_id` lets that worker id win
/// over the parent's, so the child's header names a thread the caller never
/// asked about (#6460).
fn descendant_transcripts(root: &Path) -> Vec<std::path::PathBuf> {
    let Some(dir) = root.parent() else {
        return Vec::new();
    };
    let Some(root_stem) = root.file_stem().and_then(|s| s.to_str()) else {
        return Vec::new();
    };
    let prefix = format!("{root_stem}__");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut matches: Vec<std::path::PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().and_then(|s| s.to_str()) == Some("jsonl")
                && path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|stem| stem.starts_with(&prefix))
        })
        .collect();
    matches.sort();
    matches
}

/// One thread's spend, split into the orchestrator's own and each sub-agent
/// archetype's, each counted exactly once.
#[derive(Debug, Clone, Default)]
pub(super) struct ThreadSpend {
    pub(super) root: TranscriptSpend,
    /// Keyed by archetype (`_meta.agent`), with a run count.
    pub(super) subagents: BTreeMap<String, (TranscriptSpend, usize)>,
    pub(super) updated: Option<String>,
    pub(super) found_transcript: bool,
}

/// Walk a thread's root transcripts and their descendants, totalling each file's
/// own recorded spend.
///
/// Correct-by-construction because every transcript records only what the agent
/// that owns it spent (see `session_host::codec::turn_usage`): the walk visits
/// each file once, so no token is counted twice however deep the delegation went,
/// and a child whose usage never reached its parent's in-turn ledger (#6459) is
/// still counted from its own file.
pub(super) fn thread_spend(workspace_dir: &Path, thread_id: &str) -> ThreadSpend {
    let mut out = ThreadSpend::default();
    let roots = find_root_transcripts_for_thread(workspace_dir, thread_id);
    for root in &roots {
        out.found_transcript = true;
        if let Some(transcript) = read(root) {
            let spend = transcript_spend(&transcript);
            out.root.input_tokens = out.root.input_tokens.saturating_add(spend.input_tokens);
            out.root.output_tokens = out.root.output_tokens.saturating_add(spend.output_tokens);
            out.root.cached_input_tokens = out
                .root
                .cached_input_tokens
                .saturating_add(spend.cached_input_tokens);
            out.root.cost_usd += spend.cost_usd;
            out.root.turns += spend.turns;
            // `find_root_transcripts_for_thread` returns oldest first, so the
            // last root to report a turn owns the last-turn view.
            if spend.turns > 0 {
                out.root.last_input_tokens = spend.last_input_tokens;
                out.root.last_output_tokens = spend.last_output_tokens;
            }
            if spend.context_window > 0 {
                out.root.context_window = spend.context_window;
            }
            if spend.model.is_some() {
                out.root.model = spend.model;
            }
            out.updated = Some(transcript.meta.updated);
        }
        for child in descendant_transcripts(root) {
            let Some(child_transcript) = read(&child) else {
                continue;
            };
            let spend = transcript_spend(&child_transcript);
            out.found_transcript = true;
            let entry = out
                .subagents
                .entry(child_transcript.meta.agent_name.clone())
                .or_default();
            entry.0.input_tokens = entry.0.input_tokens.saturating_add(spend.input_tokens);
            entry.0.output_tokens = entry.0.output_tokens.saturating_add(spend.output_tokens);
            entry.0.cached_input_tokens = entry
                .0
                .cached_input_tokens
                .saturating_add(spend.cached_input_tokens);
            entry.0.cost_usd += spend.cost_usd;
            entry.0.turns += spend.turns;
            if entry.0.model.is_none() {
                entry.0.model = spend.model;
            }
            entry.1 += 1;
        }
    }
    out
}

/// Total a thread's persisted token/cost usage across its root transcripts.
pub async fn token_usage(
    request: ThreadTokenUsageRequest,
) -> Result<RpcOutcome<ApiEnvelope<ThreadTokenUsageResponse>>, String> {
    let dir = workspace_dir().await?;
    let spend = thread_spend(&dir, &request.thread_id);

    // Re-audit cost at CURRENT pricing rather than trusting the
    // `charged_amount_usd` persisted in the transcript: those values were
    // stamped at turn time and don't reflect later tier-pricing corrections.
    // Recompute from the persisted token counts using the last-known model's
    // rates; falls back to `fallback` only when the model is unknown.
    let audit_cost =
        |model: Option<&str>, input: u64, output: u64, cached: u64, fallback: f64| match model {
            Some(m) => crate::agent::cost::estimate_call_cost_usd(
                m,
                &crate::inference::provider::UsageInfo {
                    input_tokens: input,
                    output_tokens: output,
                    cached_input_tokens: cached,
                    ..Default::default()
                },
            ),
            None => fallback,
        };

    if !spend.found_transcript {
        return Ok(envelope(
            empty_response(&request.thread_id),
            Some(counts([("has_usage", 0)])),
            None,
        ));
    }

    let root_model = spend.root.model.clone();
    let context_window = if spend.root.context_window > 0 {
        spend.root.context_window
    } else {
        root_model
            .as_deref()
            .and_then(crate::inference::model_context::context_window_for_model)
            .unwrap_or(0)
    };

    // Orchestrator (root) spend, re-audited.
    let orchestrator_cost = audit_cost(
        root_model.as_deref(),
        spend.root.input_tokens,
        spend.root.output_tokens,
        spend.root.cached_input_tokens,
        spend.root.cost_usd,
    );

    // Sub-agent archetypes, each re-audited with its own model. Older
    // sub-agent transcripts didn't persist a model on their messages, so
    // fall back to the thread's (root) model rather than pricing them at
    // $0 — sub-agents usually run on the same managed tier as the parent.
    let mut subagents = Vec::with_capacity(spend.subagents.len());
    let (mut sub_in, mut sub_out, mut sub_cached, mut sub_cost) = (0u64, 0u64, 0u64, 0.0);
    for (agent_id, (child, runs)) in &spend.subagents {
        let sub_model = child.model.as_deref().or(root_model.as_deref());
        let cost = audit_cost(
            sub_model,
            child.input_tokens,
            child.output_tokens,
            child.cached_input_tokens,
            child.cost_usd,
        );
        sub_in = sub_in.saturating_add(child.input_tokens);
        sub_out = sub_out.saturating_add(child.output_tokens);
        sub_cached = sub_cached.saturating_add(child.cached_input_tokens);
        sub_cost += cost;
        subagents.push(SubagentUsageDto {
            agent_id: agent_id.clone(),
            input_tokens: child.input_tokens,
            output_tokens: child.output_tokens,
            cost_usd: cost,
            runs: *runs,
        });
    }

    // Top-level totals = orchestrator + all sub-agents, each counted once
    // because each transcript recorded only its own spend.
    let input_tokens = spend.root.input_tokens.saturating_add(sub_in);
    let output_tokens = spend.root.output_tokens.saturating_add(sub_out);
    let cached_input_tokens = spend.root.cached_input_tokens.saturating_add(sub_cached);
    let cost_usd = orchestrator_cost + sub_cost;
    // A thread whose transcripts exist but recorded no spend must not claim
    // usage: the UI replaces its live bucket with this payload.
    let has_usage =
        input_tokens > 0 || output_tokens > 0 || cached_input_tokens > 0 || cost_usd > 0.0;

    let response = ThreadTokenUsageResponse {
        thread_id: request.thread_id.clone(),
        input_tokens,
        output_tokens,
        cached_input_tokens,
        cost_usd,
        turn_count: spend.root.turns,
        last_turn_input_tokens: spend.root.last_input_tokens,
        last_turn_output_tokens: spend.root.last_output_tokens,
        context_window,
        model: root_model,
        updated: spend.updated,
        has_usage,
        subagents,
    };

    Ok(envelope(
        response,
        Some(counts([("has_usage", usize::from(has_usage))])),
        None,
    ))
}

fn empty_response(thread_id: &str) -> ThreadTokenUsageResponse {
    ThreadTokenUsageResponse {
        thread_id: thread_id.to_string(),
        input_tokens: 0,
        output_tokens: 0,
        cached_input_tokens: 0,
        cost_usd: 0.0,
        turn_count: 0,
        last_turn_input_tokens: 0,
        last_turn_output_tokens: 0,
        context_window: 0,
        model: None,
        updated: None,
        has_usage: false,
        subagents: Vec::new(),
    }
}

#[cfg(test)]
#[path = "usage_tests.rs"]
mod tests;
