use super::*;

// The pure text heuristics (`text_looks_like_question` and the code-span / URL /
// paragraph checks under it) live in `tinyflows-copilot`; what stays here needs
// the host's `TranscriptEntry` history and tool names.
pub(super) use tinyflows_copilot::text_looks_like_question;

/// Builder-authoring tools whose result body can explain a trail-off — the
/// authoring belt `dry_run_workflow`/`validate_workflow`/`propose_workflow`/
/// `revise_workflow`/`edit_workflow`/`save_workflow` all report either a hard
/// gate rejection (`ToolResult::error`) or a self-reported broken-graph
/// result (`"ok": false` in a successful body), so a plain-text read-only
/// tool's output is never misattributed as the blocker.
const TRAIL_OFF_BLOCKER_TOOLS: &[&str] = &[
    "dry_run_workflow",
    "validate_workflow",
    "propose_workflow",
    "revise_workflow",
    "edit_workflow",
    "save_workflow",
];

/// Synthesizes a guaranteed, user-facing fallback for a trail-off turn (no
/// proposal, not capped, no run error, and the model's own text isn't a
/// question). Scans the run's tool history for the last builder-tool result
/// that looks like a blocker (a hard-gate rejection, or a `dry_run_workflow`/
/// `validate_workflow` report with `"ok": false`) and asks the user about it;
/// falls back to a generic "what should I focus on" question when no such
/// blocker is found (the model may have simply stopped with nothing to point
/// to).
pub(super) fn build_trail_off_fallback(
    history: &[tinytools_agent::dialect::TranscriptEntry],
) -> String {
    match last_builder_tool_blocker(history) {
        Some(blocker) => format!(
            "I wasn't able to finish building this workflow. Here's where I got stuck:\n\n{blocker}\n\n\
             Could you tell me how you'd like me to resolve that, or share more detail about what's needed here?"
        ),
        None => "I wasn't able to finish building this workflow in this turn. Could you describe \
                  what you'd like in more detail, or tell me which part to focus on?"
            .to_string(),
    }
}

/// Combines the guaranteed trail-off `fallback` question with the model's own
/// `original` text instead of discarding it (#4887 follow-up, Change 2). Even
/// after loosening `text_looks_like_question`, a future false negative must
/// never destroy the model's words — it should only ever ADD the guaranteed
/// question on top. The `fallback` is prepended (so the user sees the
/// actionable question first) and the original is kept below a divider for
/// context. When `original` is empty/whitespace-only (a genuine silent
/// turn — there's nothing to preserve), returns the fallback alone rather
/// than prepending an empty divider.
pub(super) fn combine_trail_off_fallback(fallback: &str, original: &str) -> String {
    let trimmed_original = original.trim();
    if trimmed_original.is_empty() {
        fallback.to_string()
    } else {
        format!("{fallback}\n\n---\n\n{trimmed_original}")
    }
}

/// Scans `history` in reverse for the last result from a
/// [`TRAIL_OFF_BLOCKER_TOOLS`] call that reads as a failure — a plain-text
/// error message (gate rejection), or a JSON body with `"ok": false` — and
/// returns a truncated, human-readable description of it. Tool names are
/// resolved by correlating each `ToolResults` entry's `tool_call_id` back to
/// the `AssistantToolCalls` message that issued it, so this never
/// misattributes an unrelated read-only tool's plain-text output as a
/// blocker.
fn last_builder_tool_blocker(
    history: &[tinytools_agent::dialect::TranscriptEntry],
) -> Option<String> {
    use tinytools_agent::dialect::TranscriptEntry;

    let mut call_names: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for message in history {
        if let TranscriptEntry::AssistantToolCalls { tool_calls, .. } = message {
            for call in tool_calls {
                call_names.insert(call.id.clone(), call.name.clone());
            }
        }
    }

    for message in history.iter().rev() {
        let TranscriptEntry::ToolResults(results) = message else {
            continue;
        };
        for result in results.iter().rev() {
            let Some(name) = call_names.get(&result.tool_call_id) else {
                continue;
            };
            if !TRAIL_OFF_BLOCKER_TOOLS.contains(&name.as_str()) {
                continue;
            }
            // This is the MOST RECENT authoring-belt tool result in the
            // turn (results are scanned newest-first). Whatever it reads as
            // is authoritative: a success/progress result here means any
            // earlier failure from the same tool was already resolved
            // within this turn, so we must stop at this result rather than
            // keep walking backward and surfacing a stale, already-fixed
            // blocker (see review discussion on this PR).
            return describe_tool_result_blocker(&result.content)
                .map(|desc| crate::util::truncate_with_ellipsis(&desc, 500));
        }
    }
    None
}

/// Reads one builder tool result's content as a failure description, or
/// `None` when it reads as success/progress (a `workflow_proposal` payload,
/// or an `"ok": true` report). The whole body is the description, never one
/// hardcoded field, so this stays correct regardless of which fields a given
/// tool uses to explain its failure.
fn describe_tool_result_blocker(content: &str) -> Option<String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if value.get("type").and_then(Value::as_str) == Some("workflow_proposal") {
            return None; // Success: a proposal was emitted.
        }
        if let Some(ok) = value.get("ok").and_then(Value::as_bool) {
            return if ok { None } else { Some(value.to_string()) };
        }
        // Some other structured payload with no `ok`/`type` marker this
        // function recognises — not confidently a blocker, skip it.
        return None;
    }
    // Non-JSON content: a hard-gate rejection (`ToolResult::error`) puts the
    // plain error message straight into the content — since every builder
    // tool's SUCCESS shape is JSON (a proposal or a `{ ok, ... }` report), a
    // bare string here is, by elimination, an error message.
    Some(trimmed.to_string())
}
