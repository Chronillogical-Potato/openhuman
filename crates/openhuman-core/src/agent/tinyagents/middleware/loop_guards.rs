//! Loop-guard thresholds and the repeat-call exemption shared by the
//! repeated-failure and repeat-progress breakers (issue #4463), ported verbatim
//! from the deleted `agent/harness/tool_loop.rs`. The halt-summary wording and
//! the terminal-inference recogniser live in the host copy table
//! (`inference::failure_copy`); the recoverable-failure marker list in
//! `tinyinference_llm::failure`.

// ── Loop-guard restorations (issue #4463) ────────────────────────────────────
//
// The TinyAgents migration dropped several loop breakers that the crate does not
// replace (verified against `harness::no_progress`, which tracks *failures*
// only): the recoverable-failure headroom, the terminal delegated-inference
// fast-halt (#3104), the policy-denied fast-trip, and the successful-repeat /
// identical-output guards (#4088 / #4095). These helpers + the
// [`RepeatProgressMiddleware`] below restore that behaviour seam-side, ported
// verbatim from the deleted `agent/harness/tool_loop.rs` thresholds/wording so
// the guards read identically to the legacy loop.

/// Recoverable/transient failures get more identical-retry headroom than the
/// deterministic default: a flaky network call or a timeout can succeed on a
/// later attempt once the model adapts (longer timeout, smaller batch, retry).
/// Mirrors the legacy `RECOVERABLE_REPEAT_FAILURE_THRESHOLD`.
pub(crate) const RECOVERABLE_REPEAT_FAILURE_THRESHOLD: u32 = 8;
/// Recoverable failures also get a larger *consecutive* (varied-args) no-progress
/// headroom before the breaker halts. Mirrors the legacy
/// `RECOVERABLE_NO_PROGRESS_FAILURE_THRESHOLD`.
pub(crate) const RECOVERABLE_NO_PROGRESS_FAILURE_THRESHOLD: u32 = 12;

/// Tools whose contract is to be re-invoked with identical arguments, so an
/// identical repeat is legitimate progress — not a no-progress loop. Today this
/// is `wait_subagent`, which polls a running async sub-agent and explicitly tells
/// the model to "call wait_subagent again" when a `timeout_secs` window elapses
/// while the sub-agent is still running. Without this exemption a task that
/// outlives two wait windows would have its third identical `wait_subagent`
/// halted by the no-progress breakers before it could collect the eventual
/// result. Ported from legacy `tool_loop::is_repeat_call_exempt` (Codex P1 on #4230).
pub(crate) fn is_repeat_call_exempt(tool: &str) -> bool {
    matches!(tool, "wait_subagent")
}
