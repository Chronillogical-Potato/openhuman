//! [`PromptCacheSegmentMiddleware`]: declare the turn's stable prompt prefix
//! (system prompt + tool schemas) as the harness-layout cache segments
//! (`system` / `tools`) with a content-derived request fingerprint, so the
//! crate `PromptCacheGuardMiddleware` has a prefix to protect and the provider
//! prompt-cache routing key stays stable across a thread's turns.

use async_trait::async_trait;
use sha2::{Digest, Sha256};

use tinyagents_harness::context::RunContext;
use tinyagents_harness::error::Result as TaResult;
use tinyagents_harness::middleware::Middleware;
use tinyinference_llm::message::Message as TaMessage;
use tinyinference_llm::model::{ModelRequest, PromptSegment, SegmentRole};

/// Stable SHA-256 fingerprint over canonical JSON. TinyAgents' prompt builder
/// uses the same shape for `ModelRequest::prompt_fingerprint`; OpenHuman builds
/// requests directly, so this adapter must stamp equivalent content-derived
/// segment ids and request fingerprints.
fn stable_prefix_fingerprint(value: &serde_json::Value) -> String {
    let mut hasher = Sha256::new();
    if serde_json::to_writer(&mut hasher, value).is_err() {
        hasher = Sha256::new();
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// `before_model`: declare the turn's stable prompt prefix (system prompt + tool
/// schemas) as [`PromptSegment`]s on the [`ModelRequest`] (issue #4249, 03.2).
///
/// OpenHuman assembles the request's messages/tools directly rather than through
/// the crate prompt builder, so `cache_segments` would otherwise stay empty and
/// the crate `PromptCacheGuardMiddleware` (installed immediately after this)
/// would have no prefix to protect. The segments use the harness-layout ids
/// `system` and `tools` — exactly those, and only those. The crate's
/// `refresh_prompt_cache_fingerprint` (agent_loop/run_loop.rs) recognises that
/// layout at dispatch and rebuilds `prompt_fingerprint` from the bytes actually
/// sent (system messages + tool schemas), so an unchanged system prompt +
/// tool-schema set yields the same fingerprint on every call of a thread, while
/// an injected timestamp/uuid/etc. or a changed tool schema flips it and the
/// guard records a
/// [`CacheLayoutEvent`](tinyagents_harness::cache::CacheLayoutEvent). Any
/// *other* id shape (an earlier version stamped `system:<sha>` / `tools:<sha>`)
/// is treated by the crate as a custom annotation and fingerprinted over the
/// **whole request**, which changed the provider `prompt_cache_key` on every
/// call — OpenRouter uses that key for sticky endpoint routing, so each call
/// re-rolled the endpoint and the per-endpoint prefix cache missed. This is
/// the structured, crate-native replacement for the deleted warn-only
/// `CacheAlignMiddleware` volatile-token scan (C3): the crate
/// `PromptCacheGuardMiddleware` now owns KV-cache-prefix drift detection via
/// recorded `CacheLayoutEvent`s. Read-only w.r.t. the transcript — only sets
/// `cache_segments` / `prompt_fingerprint`.
pub(crate) struct PromptCacheSegmentMiddleware;

/// Segment ids the crate's `refresh_prompt_cache_fingerprint` recognises as its
/// own stable-prefix layout. Any other id opts the request into whole-request
/// fingerprinting (see the middleware docs).
const HARNESS_SYSTEM_SEGMENT_ID: &str = "system";
const HARNESS_TOOLS_SEGMENT_ID: &str = "tools";

#[async_trait]
impl Middleware<(), crate::agent::tinyagents::host::OpenHumanRunContext>
    for PromptCacheSegmentMiddleware
{
    fn name(&self) -> &str {
        "prompt_cache_segments"
    }

    async fn before_model(
        &self,
        _ctx: &mut RunContext<crate::agent::tinyagents::host::OpenHumanRunContext>,
        _state: &(),
        request: &mut ModelRequest,
    ) -> TaResult<()> {
        let mut segments: Vec<PromptSegment> = Vec::new();
        // 1. System prompt — the cache-hottest stable prefix segment.
        if request
            .messages
            .iter()
            .any(|m| matches!(m, TaMessage::System(_)))
        {
            segments.push(PromptSegment {
                id: HARNESS_SYSTEM_SEGMENT_ID.to_string(),
                role: SegmentRole::System,
                cacheable: true,
            });
        }
        // 2. Tool schemas — advertised tool surface identity (full schemas, in
        //    registration order) forms the next stable prefix segment. A changed
        //    tool surface legitimately busts the prefix; an unchanged one keeps
        //    it stable.
        if !request.tools.is_empty() {
            segments.push(PromptSegment {
                id: HARNESS_TOOLS_SEGMENT_ID.to_string(),
                role: SegmentRole::Tools,
                cacheable: true,
            });
        }
        if !segments.is_empty() {
            // Content-derived, so a guard reading it before dispatch sees a
            // system-prompt or tool-schema edit; the crate recomputes it from
            // the final bytes at dispatch.
            let system_messages: Vec<&TaMessage> = request
                .messages
                .iter()
                .filter(|m| matches!(m, TaMessage::System(_)))
                .collect();
            request.prompt_fingerprint = Some(stable_prefix_fingerprint(&serde_json::json!({
                "system": system_messages,
                "tools": &request.tools,
            })));
            tracing::debug!(
                segment_count = segments.len(),
                fingerprint = request.prompt_fingerprint.as_deref().unwrap_or(""),
                "[cache] declared stable prompt-prefix segments for KV-cache guard"
            );
            request.cache_segments = segments;
        }
        Ok(())
    }
}
