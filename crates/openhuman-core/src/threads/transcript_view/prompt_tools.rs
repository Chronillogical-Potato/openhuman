//! Prompt-guided (text-mode) tool turns in the display projection.
//!
//! A provider without native tool calling persists a tool round differently
//! from a native one, and the projection used to read only the native shape:
//!
//! - the assistant line that issued the calls carries **no** `tool_calls`
//!   (the calls rode its visible text and were parsed out of it);
//! - the results come back as ONE `user` line, `[Tool results]` followed by a
//!   `<tool_result id="…">…</tool_result>` block per call, not as `tool` lines;
//! - the turn's calls are recorded once, on the FINAL assistant line's
//!   `tool_calls`, which is the answer rather than a line that called anything.
//!
//! Read naively that inverts the turn: every narration projected as a final
//! answer (non-interim, so the UI dropped it), the real answer projected as an
//! interim tool-calling step with every call of the turn hung after it, and
//! the `[Tool results]` line projected as something the user said. A reopened
//! thread then showed the answer twice and its tools after it — nothing like
//! the turn the user watched stream.
//!
//! This module recognises that shape so `project` can put each call right
//! after the narration that issued it, pair each result block to its call, and
//! stop re-attaching already-shown calls to the answer.

use std::collections::HashMap;

use tinyagents_session::transcript::{DisplayMessage, DisplayRecord};

/// The prefix `messages_to_text_mode_chat` / `coalesce_prompt_tool_results`
/// write on a folded tool-results user turn.
const TOOL_RESULTS_MARKER: &str = "[Tool results]";
const RESULT_OPEN: &str = "<tool_result";
const RESULT_CLOSE: &str = "</tool_result>";

/// One `<tool_result>` block of a `[Tool results]` user line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PromptToolResult {
    /// The call id, when the block carries one (`<tool_result id="…">`).
    pub id: Option<String>,
    pub body: String,
}

/// The result blocks of a `[Tool results]` user line, or `None` when the line
/// is not one. A marker with no parseable block still returns `Some(vec![])`:
/// it is tool plumbing either way and must never render as a user message.
pub(super) fn parse_tool_results(msg: &DisplayMessage) -> Option<Vec<PromptToolResult>> {
    if msg.message.role != "user" {
        return None;
    }
    let rest = msg
        .message
        .content
        .trim_start()
        .strip_prefix(TOOL_RESULTS_MARKER)?;
    let mut blocks = Vec::new();
    let mut cursor = rest;
    while let Some(start) = cursor.find(RESULT_OPEN) {
        let after_open = &cursor[start + RESULT_OPEN.len()..];
        let Some(tag_end) = after_open.find('>') else {
            break;
        };
        let id = attribute(&after_open[..tag_end], "id");
        let body_and_rest = &after_open[tag_end + 1..];
        let (body, next) = match body_and_rest.find(RESULT_CLOSE) {
            Some(end) => (
                &body_and_rest[..end],
                &body_and_rest[end + RESULT_CLOSE.len()..],
            ),
            None => (body_and_rest, ""),
        };
        blocks.push(PromptToolResult {
            id,
            body: body.trim_matches('\n').to_string(),
        });
        cursor = next;
    }
    Some(blocks)
}

/// `name="value"` out of a tag's attribute text.
fn attribute(attrs: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let start = attrs.find(&needle)? + needle.len();
    let value = &attrs[start..];
    let value = &value[..value.find('"')?];
    (!value.is_empty()).then(|| value.to_string())
}

/// `(name, arguments)` of every call a file records, by call id, plus each
/// turn's call ids in issue order (for id-less result blocks).
#[derive(Debug, Default)]
pub(super) struct CallRegistry {
    by_id: HashMap<String, (String, String)>,
    by_turn: HashMap<Option<String>, Vec<String>>,
}

impl CallRegistry {
    /// Index the calls every assistant line records (native per-line calls,
    /// and the turn-level list a prompt-guided turn stamps on its answer).
    pub(super) fn from_records(records: &[DisplayRecord]) -> Self {
        let mut registry = Self::default();
        for record in records {
            let DisplayRecord::Message(msg) = record else {
                continue;
            };
            if msg.message.role != "assistant" {
                continue;
            }
            let Some(usage) = msg.turn_usage.as_ref() else {
                continue;
            };
            for call in &usage.tool_calls {
                if call.id.is_empty() || registry.by_id.contains_key(&call.id) {
                    continue;
                }
                registry
                    .by_id
                    .insert(call.id.clone(), (call.name.clone(), call.arguments.clone()));
                registry
                    .by_turn
                    .entry(msg.request_id.clone())
                    .or_default()
                    .push(call.id.clone());
            }
        }
        registry
    }

    /// The calls a `[Tool results]` line answers, as `(id, name, arguments)`,
    /// in block order. A block with an id resolves by it; an id-less block
    /// takes the turn's next call not yet `emitted`. Unknown ids keep their id
    /// with the generic name `tool`, so the result still lands on a row.
    pub(super) fn calls_for(
        &self,
        request_id: &Option<String>,
        blocks: &[PromptToolResult],
        emitted: &std::collections::HashSet<String>,
    ) -> Vec<(String, String, String)> {
        let mut taken: Vec<String> = Vec::new();
        let mut turn_order = self
            .by_turn
            .get(request_id)
            .into_iter()
            .flatten()
            .filter(|id| !emitted.contains(*id));
        blocks
            .iter()
            .map(|block| {
                let id = match &block.id {
                    Some(id) => id.clone(),
                    None => turn_order
                        .find(|id| !taken.contains(id))
                        .cloned()
                        .unwrap_or_default(),
                };
                taken.push(id.clone());
                let (name, arguments) = self
                    .by_id
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| ("tool".to_string(), String::new()));
                (id, name, arguments)
            })
            .collect()
    }
}
