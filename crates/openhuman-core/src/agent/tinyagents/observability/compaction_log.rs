//! Journal projection of `AgentEvent::Compacted`.

use tinyagents_harness::events::AgentEvent;

/// Logs a context compaction at info with its token savings and the
/// summarizer call's latency and usage (grep `context compacted`).
///
/// A compaction rewrites what the model sees and spends a summarizer call
/// outside the turn's own model calls, so it is worth seeing at the default
/// log level, with that call's cost.
pub(super) fn log_compacted(event: &AgentEvent) {
    let AgentEvent::Compacted {
        reason,
        tokens_before,
        tokens_after,
        usage,
        latency_ms,
    } = event
    else {
        return;
    };
    tracing::info!(
        reason = reason.as_str(),
        tokens_before,
        tokens_after,
        saved_tokens = tokens_before.saturating_sub(*tokens_after),
        latency_ms = ?latency_ms,
        summarizer_input_tokens = ?usage.map(|u| u.input_tokens),
        summarizer_output_tokens = ?usage.map(|u| u.output_tokens),
        summarizer_cached_tokens = ?usage.map(|u| u.cache_read_tokens),
        "[tinyagents] context compacted"
    );
}
