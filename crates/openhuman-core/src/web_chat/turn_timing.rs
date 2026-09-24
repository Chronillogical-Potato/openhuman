//! Time-to-first-visible instrumentation for a web-chat turn.
//!
//! A turn that shows nothing for 40 s looks the same in the logs as one that
//! streams a lead-in at 5 s unless the first text delta and the first tool
//! call are stamped against the turn start. `progress_bridge` feeds this from
//! the progress stream; grep `time-to-first-visible` to read it back.

use std::time::Instant;

pub(super) struct TurnTiming {
    started: Instant,
    first_text_ms: Option<u128>,
    first_tool_ms: Option<u128>,
    round_one_narration_chars: usize,
}

impl TurnTiming {
    pub(super) fn start() -> Self {
        Self {
            started: Instant::now(),
            first_text_ms: None,
            first_tool_ms: None,
            round_one_narration_chars: 0,
        }
    }

    /// A text delta arrived; the first non-blank one is the first visible byte.
    pub(super) fn text_delta(&mut self, delta: &str, round: u32, request_id: &str) {
        if self.first_text_ms.is_none() && !delta.trim().is_empty() {
            let elapsed = self.started.elapsed().as_millis();
            self.first_text_ms = Some(elapsed);
            log::info!(
                "[web_channel][bridge] time-to-first-visible kind=text first_text_ms={elapsed} round={round} request_id={request_id}"
            );
        }
        if round <= 1 {
            self.round_one_narration_chars += delta.chars().count();
        }
    }

    /// A tool call started; the first one closes the model's first response.
    pub(super) fn tool_call(&mut self, tool_name: &str, round: u32, request_id: &str) {
        if self.first_tool_ms.is_some() {
            return;
        }
        let elapsed = self.started.elapsed().as_millis();
        self.first_tool_ms = Some(elapsed);
        log::info!(
            "[web_channel][bridge] time-to-first-visible kind=tool_call first_tool_ms={elapsed} first_text_ms={:?} round={round} tool={tool_name} request_id={request_id}",
            self.first_text_ms
        );
    }

    /// The turn finished: one summary line with both firsts and the total.
    pub(super) fn done(&self, iterations: u32, interim_threshold: usize, request_id: &str) {
        log::info!(
            "[web_channel][bridge] time-to-first-visible kind=turn_done total_ms={} first_text_ms={:?} first_tool_ms={:?} round_one_narration_chars={} interim_threshold={interim_threshold} iterations={iterations} request_id={request_id}",
            self.started.elapsed().as_millis(),
            self.first_text_ms,
            self.first_tool_ms,
            self.round_one_narration_chars
        );
    }

    /// A point-in-time copy of this turn's timing, for carrying across the
    /// `ProgressBridgeHandle` → `WebChatTaskResult` → `chat_done.timing`
    /// pipeline (the bridge task itself never outlives the turn it times, so
    /// the `Instant` stays put — only the derived millisecond counts leave
    /// this module).
    pub(super) fn snapshot(&self) -> TurnTimingSnapshot {
        TurnTimingSnapshot {
            first_token_ms: self.first_text_ms.map(|ms| ms as u64),
            first_tool_ms: self.first_tool_ms.map(|ms| ms as u64),
            total_ms: Some(self.started.elapsed().as_millis() as u64),
        }
    }
}

/// Plain-data copy of a turn's timing, safe to hand across an `Arc<Mutex<_>>`
/// out of the bridge task and into `chat_done.timing`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TurnTimingSnapshot {
    pub(crate) first_token_ms: Option<u64>,
    pub(crate) first_tool_ms: Option<u64>,
    pub(crate) total_ms: Option<u64>,
}

impl TurnTimingSnapshot {
    /// Convert to the wire payload, filling `tokens_per_second` from
    /// `output_tokens` when both it and `total_ms` are available and
    /// `total_ms` is non-zero (avoids a division-by-zero / infinity on an
    /// instantaneous synthetic result).
    pub(crate) fn into_payload(
        self,
        output_tokens: Option<u64>,
    ) -> crate::core::socketio::TurnTimingPayload {
        let tokens_per_second = match (output_tokens, self.total_ms) {
            (Some(tokens), Some(total_ms)) if total_ms > 0 => {
                Some(tokens as f64 / (total_ms as f64 / 1000.0))
            }
            _ => None,
        };
        crate::core::socketio::TurnTimingPayload {
            first_token_ms: self.first_token_ms,
            first_tool_ms: self.first_tool_ms,
            total_ms: self.total_ms,
            tokens_per_second,
        }
    }
}
