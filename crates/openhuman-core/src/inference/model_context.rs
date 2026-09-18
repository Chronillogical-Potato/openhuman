//! Known model context-window sizes for pre-inference budgeting.
//!
//! Provider `/models` responses may include `context_length` / `context_window`,
//! but the agent harness must enforce limits **before** the first dispatch —
//! otherwise long histories produce upstream `400 Bad Request` errors when usage
//! metadata is not yet available.

use crate::config::{
    MODEL_AGENTIC_V1, MODEL_BURST_V1, MODEL_CHAT_V1, MODEL_CODING_V1, MODEL_REASONING_QUICK_V1,
    MODEL_REASONING_V1,
};

/// Conservative default for OpenHuman abstract tier models (tokens).
const TIER_LARGE_CONTEXT: u64 = 200_000;
/// Reasoning tier — backed by a 1M-context model.
const TIER_REASONING_CONTEXT: u64 = 1_000_000;
const TIER_STANDARD_CONTEXT: u64 = 128_000;
const TIER_LOCAL_CONTEXT: u64 = 8_192;

/// DeepSeek v4 Flash window (~1M tokens) — the shared backing for every managed
/// "flash" tier: `chat-v1`, its legacy alias `reasoning-quick-v1`, and
/// `summarization-v1`. Kept as a single constant so these tiers can't drift
/// apart again (issue #4706: `chat-v1` was pinned at `TIER_STANDARD_CONTEXT`
/// (128K) while `summarization-v1` was 1M, even though both resolve to DeepSeek
/// v4 Flash in the backend model registry). `extract_from_result` also relies on
/// this window to single-shot whole oversized payloads instead of chunking, so
/// it must reflect the real backing model's capacity.
const TIER_FLASH_CONTEXT: u64 = 1_000_000;

/// Resolve the context window (in tokens) for a model id or OpenHuman tier alias.
///
/// Returns `None` when the model is unknown — callers should skip pre-dispatch
/// trimming rather than guess.
pub fn context_window_for_model(model: &str) -> Option<u64> {
    let normalized = model.trim();
    if normalized.is_empty() {
        return None;
    }

    if let Some(window) = tier_context_window(normalized) {
        return Some(window);
    }

    if let Some(price) = crate::platform::cost::catalog::lookup(normalized) {
        tracing::debug!(
            model = normalized,
            catalog_model = price.model_id,
            context_window = price.context_window,
            "[model_context] matched cost catalog row"
        );
        return Some(u64::from(price.context_window));
    }

    if let Some(window) = tinyinference_core::model::context_window_for_model_id(normalized) {
        tracing::debug!(
            model = normalized,
            context_window = window,
            "[model_context] matched tinyagents model context hint"
        );
        return Some(window);
    }

    None
}

fn tier_context_window(model: &str) -> Option<u64> {
    match model {
        MODEL_REASONING_V1 => Some(TIER_REASONING_CONTEXT),
        MODEL_AGENTIC_V1 | MODEL_CODING_V1 => Some(TIER_LARGE_CONTEXT),
        "summarization-v1" => Some(TIER_FLASH_CONTEXT),
        // Burst tier advertises a 128k window on the managed backend. Matched on
        // the `burst-v1` alias before any substring fallbacks below.
        MODEL_BURST_V1 => Some(TIER_STANDARD_CONTEXT),
        // `chat-v1` (and its legacy alias `reasoning-quick-v1`) are backed by
        // DeepSeek v4 Flash — the same ~1M model as `summarization-v1`, not a
        // 128K model (issue #4706). Share `TIER_FLASH_CONTEXT` so the three
        // flash tiers stay in lockstep.
        MODEL_CHAT_V1 | MODEL_REASONING_QUICK_V1 | "chat" => Some(TIER_FLASH_CONTEXT),
        m if m.starts_with("gemma") || m.contains(":1b") || m.contains("270m") => {
            Some(TIER_LOCAL_CONTEXT)
        }
        _ => None,
    }
}

/// Whether the model resolved for a chat hint/agent/profile accepts image input
/// according to the **user-configured** vision flag in `config.model_registry`.
///
/// This is the per-model override that lets a user mark a **custom / BYOK** model
/// as vision-capable (Settings → Advanced LLM → custom model → "Supports
/// vision"). Managed-backend models already advertise vision via
/// [`crate::inference::provider::Provider::supports_vision`]; this flag
/// covers OpenAI-compatible providers the backend can't introspect per-model.
/// Returns `false` for models the user has not flagged.
pub fn model_vision_enabled(model: &str, config: &crate::config::Config) -> bool {
    let normalized = model.trim();
    if normalized.is_empty() {
        return false;
    }
    let enabled = config
        .model_registry
        .iter()
        .any(|entry| entry.id == normalized && entry.vision);
    tracing::debug!(
        model = normalized,
        vision_enabled = enabled,
        "[model_context] resolved user-configured vision flag"
    );
    enabled
}

/// Whether a resolved model accepts image input. The single predicate shared by
/// the chat UI resolve and the server-side session/sub-agent gates.
///
/// - **Managed OpenHuman tiers** consult the hardcoded per-tier map
///   ([`crate::inference::provider::factory::oh_tier_supports_vision`]) —
///   the remote backend does not advertise per-tier capability, so the core owns
///   it. Currently `reasoning-v1` and `vision-v1` — plus their `hint:reasoning`
///   / `hint:vision` aliases — are vision-capable; every other tier is not.
/// - **Custom/BYOK models** consult the user-set `model_registry.vision` flag
///   ([`model_vision_enabled`]).
pub fn model_supports_vision(model: &str, config: &crate::config::Config) -> bool {
    crate::inference::provider::factory::oh_tier_supports_vision(model)
        || model_vision_enabled(model, config)
}

#[cfg(test)]
#[path = "model_context_tests.rs"]
mod tests;
