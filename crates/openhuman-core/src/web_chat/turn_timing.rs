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
}
