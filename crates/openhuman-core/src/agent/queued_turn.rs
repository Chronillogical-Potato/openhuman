//! Host-owned payload for a message queued while an agent turn is active.
//!
//! TinyAgents owns the queue mechanics and its [`QueueLane`](tinyagents_harness::run_queue::QueueLane)
//! selects how this payload is consumed. OpenHuman retains the web and
//! orchestration metadata needed to dispatch a deferred follow-up turn.

/// A queued OpenHuman turn input.
///
/// The payload deliberately does not carry a lane: lane selection is made at
/// the host boundary when the item is pushed into TinyAgents' `RunQueue`.
#[derive(Debug, Clone)]
pub struct QueuedTurn {
    /// Stable id for this queued item (minted once, at push time). Carried on
    /// `RunQueue*` domain events (`item_id`) and the `queue_item_*` web-channel
    /// events so the frontend can key a queued-message row and later target it
    /// with `channel.web_queue_remove`.
    pub id: String,
    pub text: String,
    pub client_id: String,
    pub thread_id: String,
    pub queued_at_ms: u64,
    pub model_override: Option<String>,
    pub temperature: Option<f64>,
    pub locale: Option<String>,
}

/// Clip a queued message's text to a short, non-sensitive preview for
/// `RunQueue*` domain events and `queue_item_*` web-channel events — never
/// the raw message body at full length.
#[must_use]
pub fn text_preview(text: &str) -> String {
    crate::core::events::clip_to_chars(text, 80)
}
