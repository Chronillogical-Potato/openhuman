//! A named output file that was never written, caught before the turn ends.
//!
//! # Why this exists
//!
//! The turn already carries four rungs that *tell* the model to produce its
//! deliverable: the budget notice part-way through, the penultimate call's
//! narrowed writer belt, the concluding call's instruction, and the
//! requirements check. Every one of them is advice, and the orchestrator
//! prompt's rule ("when the request names an output file, that file is the
//! deliverable") is advice too. None of them observes whether anything was
//! written, so a turn that ignores all five ends with a careful account of
//! work nobody can use.
//!
//! That is not hypothetical. One run reconstructed a transcript, catalogued
//! eleven variants and resolved the domain it was asked for, listed the output
//! file in its own answer as "still outstanding", and ended the turn. Every
//! check that read the file failed on the file's absence rather than on
//! anything it contained; an earlier run of the same request, which wrote a
//! partly-filled file, was scored on its contents.
//!
//! # How a path is identified without parsing the request
//!
//! The obvious objection to reading paths out of a request is telling an
//! output from an input: a request routinely names both. Grammar ("write to"
//! versus "read from") would be the fragile way to decide.
//!
//! The existence check decides it instead, for free. A path the request names
//! as an input is already on disk, so it is never reported. A path that the
//! request names and that nothing has created is, by construction, either a
//! deliverable that was skipped or a path the request mentioned in passing —
//! and the cost of the second case is one advisory sentence.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use async_trait::async_trait;
use tinyagents_harness::context::RunContext;
use tinyagents_harness::error::{Result, TinyAgentsError};
use tinyagents_harness::middleware::Middleware;
use tinyagents_harness::tinyinference_llm::message::Message;
use tinyagents_harness::tinyinference_llm::model::{ModelRequest, ModelResponse};

use crate::agent::session_host::turn_checkpoint::wrap_harness_instruction;

/// Leave the model room to act on the notice. Below this the turn cannot both
/// write a file and answer, so the notice would only cost a call.
const MIN_REMAINING_MODEL_CALLS: usize = 3;

/// How many candidates one request may contribute. A request naming more
/// absolute paths than this is describing a tree, not a deliverable, and
/// statting an unbounded list on the concluding call is not worth it.
const MAX_CANDIDATES: usize = 8;

/// Longest plausible path. Anything longer is prose that happens to contain
/// slashes.
const MAX_PATH_CHARS: usize = 200;

/// Characters that continue a path token. Everything else — whitespace,
/// quotes, backticks, brackets, commas — ends it.
fn is_path_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | '+' | '@' | '%')
}

/// Whether `segment` ends in a file extension: a dot followed by 1-8
/// alphanumerics, and not the whole segment (so a dotfile is not an
/// extension).
fn has_extension(segment: &str) -> bool {
    match segment.rsplit_once('.') {
        Some((stem, ext)) => {
            !stem.is_empty()
                && (1..=8).contains(&ext.len())
                && ext.chars().all(|c| c.is_ascii_alphanumeric())
        }
        None => false,
    }
}

/// Absolute paths with a file extension that `text` names, in order, deduped
/// and capped at [`MAX_CANDIDATES`].
///
/// Absolute only: a relative token in prose ("see config/settings.yml") is far
/// more often a reference than a deliverable, and a relative path cannot be
/// resolved without assuming the turn's working directory. An extension is
/// required for the same reason — it is what separates a file the request asks
/// for from a directory or a sentence fragment. Both limits mean this reports
/// nothing rather than guessing when a request is written loosely.
pub(crate) fn candidate_paths(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let bytes: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != '/' {
            index += 1;
            continue;
        }
        // A path starts at a `/` that does not continue a token (so `a/b` in
        // prose is not read as rooted at `/b`).
        if index > 0 && is_path_char(bytes[index - 1]) {
            index += 1;
            continue;
        }
        let start = index;
        while index < bytes.len() && is_path_char(bytes[index]) {
            index += 1;
        }
        let token: String = bytes[start..index].iter().collect();
        let token = token.trim_end_matches(['.', '-', '_', '+']);
        if token.len() < 3 || token.len() > MAX_PATH_CHARS {
            continue;
        }
        // `..` is never statted: a request that needs traversal to name its
        // own output is not a case this should act on.
        if token.contains("..") {
            continue;
        }
        let mut segments = token.split('/').filter(|s| !s.is_empty());
        let Some(first) = segments.next() else {
            continue;
        };
        let Some(last) = token.rsplit('/').next() else {
            continue;
        };
        // At least two segments: `/report.json` at the filesystem root is far
        // more likely a stray token than a deliverable.
        if first == last || !has_extension(last) {
            continue;
        }
        if seen.insert(token.to_string()) {
            out.push(token.to_string());
            if out.len() >= MAX_CANDIDATES {
                break;
            }
        }
    }
    out
}

/// The notice, naming every candidate that does not exist.
pub(crate) fn notice(missing: &[String]) -> String {
    let list = missing
        .iter()
        .map(|path| format!("`{path}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let (subject, verb) = if missing.len() == 1 {
        ("a file", "does not exist")
    } else {
        ("files", "do not exist")
    };
    wrap_harness_instruction(&format!(
        "The request names {subject} that {verb}: {list}. Nothing has been written there, so \
         none of this turn's work is available to whoever asked for it. Write it now with \
         whatever you have established, even where fields are incomplete or provisional, and \
         then answer. A file holding the parts you are sure of is worth more than a \
         description of what it would have contained; mark anything provisional inside it, or \
         say in your reply what is still missing."
    ))
}

/// Per-run state: the candidates read off the request, and whether the notice
/// has already been given.
#[derive(Default)]
struct RunState {
    candidates: Option<Vec<String>>,
    fired: bool,
}

/// Holds a turn's first final answer once, when the request named an output
/// file that nothing has created. See the module docs.
pub(crate) struct UnmetDeliverableMiddleware {
    runs: Mutex<HashMap<u64, RunState>>,
}

impl UnmetDeliverableMiddleware {
    pub(crate) fn new() -> Self {
        Self {
            runs: Mutex::default(),
        }
    }

    /// Which of `candidates` are absent. Existence only: nothing is opened or
    /// read, and the paths echoed back are the ones the request already
    /// carried.
    fn missing(candidates: &[String]) -> Vec<String> {
        candidates
            .iter()
            .filter(|path| !std::path::Path::new(path.as_str()).exists())
            .cloned()
            .collect()
    }

    /// Why this response must not be held, or `None` when it may be.
    fn skip_reason<C>(ctx: &RunContext<C>, response: &ModelResponse) -> Option<&'static str> {
        if !response.tool_calls().is_empty() {
            return Some("not_final");
        }
        if response.text().trim().is_empty() {
            return Some("empty_answer");
        }
        if response.finish_reason.as_deref() == Some("length") {
            return Some("truncated");
        }
        if response.continue_turn.is_some() {
            return Some("already_continued");
        }
        if ctx.limits.remaining_model_calls() < MIN_REMAINING_MODEL_CALLS {
            return Some("model_call_budget");
        }
        None
    }
}

#[async_trait]
impl<C: Send + Sync> Middleware<(), C> for UnmetDeliverableMiddleware {
    fn name(&self) -> &str {
        "unmet_deliverable"
    }

    async fn before_model(
        &self,
        ctx: &mut RunContext<C>,
        _state: &(),
        request: &mut ModelRequest,
    ) -> Result<()> {
        let Ok(mut runs) = self.runs.lock() else {
            return Ok(());
        };
        let run = runs.entry(ctx.instance_id()).or_default();
        if run.candidates.is_some() {
            return Ok(());
        }
        // The request is the turn's first user message; later user turns are
        // this harness's own injections and the model's follow-ups.
        let text = request
            .messages
            .iter()
            .find(|message| matches!(message, Message::User(_)))
            .map(Message::text)
            .unwrap_or_default();
        run.candidates = Some(candidate_paths(&text));
        Ok(())
    }

    async fn after_model(
        &self,
        ctx: &mut RunContext<C>,
        _state: &(),
        response: &mut ModelResponse,
    ) -> Result<()> {
        let candidates = {
            let Ok(runs) = self.runs.lock() else {
                return Ok(());
            };
            match runs.get(&ctx.instance_id()) {
                Some(run) if !run.fired => run.candidates.clone().unwrap_or_default(),
                _ => return Ok(()),
            }
        };
        if candidates.is_empty() {
            return Ok(());
        }
        if let Some(reason) = Self::skip_reason(ctx, response) {
            tracing::debug!(reason, "[unmet_deliverable] not holding this answer");
            return Ok(());
        }
        let missing = Self::missing(&candidates);
        if missing.is_empty() {
            return Ok(());
        }
        if let Ok(mut runs) = self.runs.lock() {
            runs.entry(ctx.instance_id()).or_default().fired = true;
        }
        tracing::info!(
            missing = missing.len(),
            "[unmet_deliverable] holding the answer: a named output file was never written"
        );
        response.continue_turn = Some(notice(&missing));
        Ok(())
    }

    async fn on_error(&self, ctx: &mut RunContext<C>, _error: &TinyAgentsError) -> Result<()> {
        if let Ok(mut runs) = self.runs.lock() {
            runs.remove(&ctx.instance_id());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "unmet_deliverable_tests.rs"]
mod tests;
