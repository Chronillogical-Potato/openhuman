//! Workload routing and model-call middleware for native TinyAgents models.

use std::sync::LazyLock;

use async_trait::async_trait;
use tinyagents_harness::context::RunContext;
use tinyagents_harness::events::AgentEvent;
use tinyagents_harness::middleware::{MiddlewareModelOutcome, ModelHandler, ModelMiddleware};
use tinyagents_harness::retry::FallbackPolicy;
use tinyagents_registry::{WorkloadRoute, WorkloadRouter};
use tinyinference_llm::model::{CapabilitySet, ModelRequest};

/// Role aliases the workload router is keyed by: the `hint:<role>` string a
/// caller pins (an agent manifest's `hint = "coding"`, a flow node's
/// `config.model`). The inference provider factory resolves each alias to the
/// role's configured route — the managed default model, or the BYOK/local
/// model routed to that workload.
pub(super) const ROUTE_CHAT: &str = "hint:chat";
pub(super) const ROUTE_REASONING: &str = "hint:reasoning";
pub(super) const ROUTE_AGENTIC: &str = "hint:agentic";
pub(super) const ROUTE_CODING: &str = "hint:coding";
pub(super) const ROUTE_BURST: &str = "hint:burst";
pub(super) const ROUTE_SUMMARIZATION: &str = "hint:summarization";
pub(super) const ROUTE_VISION: &str = "hint:vision";

/// The workload routes projected into the registry, keyed by role alias.
///
/// This is the canonical workload inventory (`chat`, `reasoning`, `agentic`,
/// `coding`, `burst`, `summarization`, `vision`). `subconscious`/`memory` are
/// intentionally absent — they are role aliases that ride the chat route rather
/// than distinct router entries.
pub(super) const WORKLOAD_ROUTE_TIERS: &[&str] = &[
    ROUTE_CHAT,
    ROUTE_REASONING,
    ROUTE_AGENTIC,
    ROUTE_CODING,
    ROUTE_BURST,
    ROUTE_SUMMARIZATION,
    ROUTE_VISION,
];

/// The OpenHuman workload-tier routing table as a crate
/// [`WorkloadRouter`](tinyagents_registry::WorkloadRouter) — the single declarative
/// source for cross-route **fallback chains** and per-tier **required-capability
/// gates** (issue #4249, Phase 3 routing consolidation).
///
/// The router owns the policy this module previously open-coded as
/// `same_family_fallbacks` +
/// `turn_required_capabilities`: it answers [`route_fallback_policy`] and
/// [`turn_required_capabilities`] from one declarative table.
///
/// Built once — the route set + fallback ordering + vision gate are static:
/// - light/fast conversational siblings `chat ⇄ burst`;
/// - heavy reasoning/agentic siblings `reasoning ⇄ agentic`;
/// - `coding → agentic` (coding is tool-heavy, agentic-adjacent);
/// - `summarization → chat` (summarization rides a general chat model);
/// - `vision` is `image_in`-gated and primary-only — a text fallback cannot
///   satisfy the gate.
///
/// On the managed backend every role resolves to the same default model, so a
/// sibling fallback there re-dispatches the same model; the chain earns its
/// keep when a role is routed to a BYOK/local provider that fails.
static OH_WORKLOAD_ROUTER: LazyLock<WorkloadRouter> = LazyLock::new(|| {
    let vision_gate = CapabilitySet {
        image_in: true,
        ..CapabilitySet::default()
    };
    WorkloadRouter::new()
        .with_route(WorkloadRoute::new(ROUTE_CHAT, ROUTE_CHAT).with_fallbacks([ROUTE_BURST]))
        .with_route(WorkloadRoute::new(ROUTE_BURST, ROUTE_BURST).with_fallbacks([ROUTE_CHAT]))
        .with_route(
            WorkloadRoute::new(ROUTE_REASONING, ROUTE_REASONING).with_fallbacks([ROUTE_AGENTIC]),
        )
        .with_route(
            WorkloadRoute::new(ROUTE_AGENTIC, ROUTE_AGENTIC).with_fallbacks([ROUTE_REASONING]),
        )
        .with_route(WorkloadRoute::new(ROUTE_CODING, ROUTE_CODING).with_fallbacks([ROUTE_AGENTIC]))
        .with_route(
            WorkloadRoute::new(ROUTE_SUMMARIZATION, ROUTE_SUMMARIZATION)
                .with_fallbacks([ROUTE_CHAT]),
        )
        // Vision is image_in-gated with no fallback (primary-only).
        .with_route(WorkloadRoute::new(ROUTE_VISION, ROUTE_VISION).requiring(vision_gate))
});

/// The capability needs a turn imposes on every model call, derived from what is
/// cheaply available at harness-assembly time.
///
/// Today the only reliably-derivable, safe-to-require signal is **vision**: when
/// the turn's effective model is the dedicated `hint:vision` tier the turn was
/// routed there because it carries image input (this is exactly what the
/// `model_vision` selection in `subagent_host/ops/graph.rs` encodes), so we
/// require `image_in` — which keeps the primary vision model selectable while
/// filtering any non-vision fallback pre-dispatch.
///
/// Returns `None` (install no gate) when no requirement is derivable, so the
/// common text turn is unaffected. Signals still to thread (see module note and
/// the migration spec): per-call tool-calling and reasoning needs, BYOK vision
/// (needs `Config` + `model_registry.vision`), and true per-message image
/// presence rather than the tier proxy.
pub(super) fn turn_required_capabilities(model: &str) -> Option<CapabilitySet> {
    OH_WORKLOAD_ROUTER.required_capabilities(model)
}

/// Around-model middleware that stamps the turn's required [`CapabilitySet`] onto
/// every [`ModelRequest`] before resolution/dispatch, so the crate rejects an
/// unfit model pre-dispatch (and, once fallback is wired in 02.2, selects the
/// next capable route) instead of failing at the provider.
///
/// It only sets the requirement when the request carries none, so an inner layer
/// that already declared stricter needs wins.
pub(super) struct RequiredCapabilitiesMiddleware {
    required: CapabilitySet,
}

impl RequiredCapabilitiesMiddleware {
    pub(super) fn new(required: CapabilitySet) -> Self {
        Self { required }
    }
}

#[async_trait]
impl ModelMiddleware<(), crate::agent::tinyagents::host::OpenHumanRunContext>
    for RequiredCapabilitiesMiddleware
{
    fn name(&self) -> &str {
        "openhuman.required_capabilities"
    }

    async fn wrap_model(
        &self,
        ctx: &mut RunContext<crate::agent::tinyagents::host::OpenHumanRunContext>,
        state: &(),
        mut request: ModelRequest,
        next: ModelHandler<'_, (), crate::agent::tinyagents::host::OpenHumanRunContext>,
    ) -> tinyagents_harness::Result<MiddlewareModelOutcome> {
        if request.required_capabilities.is_none() {
            request = request.with_required_capabilities(self.required.clone());
        }
        next.run(ctx, state, request).await
    }
}

/// Build the [`FallbackPolicy`] for a turn whose effective/primary model is
/// `model` (issue #4249, Workstream 02.2). The returned chain is `[primary,
/// alternate…]` — the crate's [`FallbackPolicy::next_after`] traversal expects the
/// current (primary) name as the first entry and yields each subsequent alternate.
///
/// The chain now comes straight from the declarative [`OH_WORKLOAD_ROUTER`]
/// (`fallback_policy` leads with the primary, then the tier's same-family
/// alternates). Returns `None` when no same-family alternate exists (vision, or
/// a raw non-tier model string), leaving the turn primary-only.
pub(super) fn route_fallback_policy(model: &str) -> Option<FallbackPolicy> {
    let policy = OH_WORKLOAD_ROUTER.fallback_policy(model);
    match &policy {
        Some(p) => tracing::debug!(
            route = model,
            chain = ?p.models,
            "[fallback] configured SDK-owned cross-route fallback chain"
        ),
        None => tracing::debug!(
            route = model,
            "[fallback] no same-family fallback route; turn is primary-only"
        ),
    }
    policy
}

/// Around-model middleware that makes the crate's registry-backed
/// [`RunPolicy::fallback`][tinyagents_harness::runtime::RunPolicy] traversal
/// **event-visible** (issue #4249, Workstream 02.2).
///
/// The harness performs the cross-route fallback swap inside its model-resolving
/// core (`agent_loop::invoke_model_resolving`) but — unlike the
/// [`ModelFallbackMiddleware`][tinyagents_harness::middleware::ModelFallbackMiddleware]
/// primitive — that native path emits **no**
/// [`AgentEvent::FallbackSelected`]. This observer wraps the resolving core, and
/// on success compares the response's `resolved_model` against the turn's primary
/// model name: when they differ a fallback occurred, so it emits the parity
/// `FallbackSelected` event (mirrored onto OpenHuman's progress/observability
/// bridge) and logs it under `[fallback]`. It never re-issues the call, so it adds
/// no extra provider dispatch on top of the native traversal (no double-fallback).
pub(super) struct FallbackObserverMiddleware {
    primary: String,
}

impl FallbackObserverMiddleware {
    pub(super) fn new(primary: impl Into<String>) -> Self {
        Self {
            primary: primary.into(),
        }
    }
}

#[async_trait]
impl ModelMiddleware<(), crate::agent::tinyagents::host::OpenHumanRunContext>
    for FallbackObserverMiddleware
{
    fn name(&self) -> &str {
        "openhuman.fallback_observer"
    }

    async fn wrap_model(
        &self,
        ctx: &mut RunContext<crate::agent::tinyagents::host::OpenHumanRunContext>,
        state: &(),
        request: ModelRequest,
        next: ModelHandler<'_, (), crate::agent::tinyagents::host::OpenHumanRunContext>,
    ) -> tinyagents_harness::Result<MiddlewareModelOutcome> {
        let outcome = next.run(ctx, state, request).await?;
        let response = outcome.into_response();
        if let Some(resolved) = response.resolved_model.as_ref() {
            if resolved.name != self.primary {
                tracing::info!(
                    from = %self.primary,
                    to = %resolved.name,
                    "[fallback] SDK selected a cross-route fallback model after the primary route failed"
                );
                ctx.emit(AgentEvent::FallbackSelected {
                    from: self.primary.clone(),
                    to: resolved.name.clone(),
                });
            }
        }
        Ok(MiddlewareModelOutcome::from(response))
    }
}

/// Around-model middleware that feeds the cost event bridge (issue #4249,
/// Phase 5): after the real model call, it reads the full host [`UsageInfo`] off
/// the returned [`ModelResponse`] — token breakdowns from the crate `Usage`,
/// backend-charged USD + context window from the G1 `raw` passthrough
/// ([`usage_info_from_response`](super::model::usage_info_from_response)) — and
/// pushes it onto the shared [`ProviderUsageCarry`](super::observability::ProviderUsageCarry)
/// the [`OpenhumanEventBridge`](super::OpenhumanEventBridge) drains on
/// `UsageRecorded`.
///
/// It wraps the whole retry/fallback core, so it fires
/// exactly once per logical model call (matching the single `UsageRecorded` the
/// crate emits), for both the buffered and streamed paths (the streamed response
/// is folded back to a `ModelResponse` with usage + raw intact). Push happens
/// after the call returns, before the loop emits `UsageRecorded`, preserving the
/// FIFO ordering the bridge relies on.
pub(super) struct UsageCarryMiddleware {
    carry: super::observability::ProviderUsageCarry,
}

impl UsageCarryMiddleware {
    pub(super) fn new(carry: super::observability::ProviderUsageCarry) -> Self {
        Self { carry }
    }
}

#[async_trait]
impl ModelMiddleware<(), crate::agent::tinyagents::host::OpenHumanRunContext>
    for UsageCarryMiddleware
{
    fn name(&self) -> &str {
        "openhuman.usage_carry"
    }

    async fn wrap_model(
        &self,
        ctx: &mut RunContext<crate::agent::tinyagents::host::OpenHumanRunContext>,
        state: &(),
        request: ModelRequest,
        next: ModelHandler<'_, (), crate::agent::tinyagents::host::OpenHumanRunContext>,
    ) -> tinyagents_harness::Result<MiddlewareModelOutcome> {
        let outcome = next.run(ctx, state, request).await?;
        let response = outcome.into_response();
        if let Some(usage) = super::model::usage_info_from_response(&response) {
            self.carry
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push_back(usage);
        }
        Ok(MiddlewareModelOutcome::from(response))
    }
}

/// Captures the concrete provider/model/host-route selected for a successful
/// call from TinyInference's canonical response metadata.
///
/// `tinyinference_llm::model::RouteRecordingModel` decorates both unary
/// responses and stream terminal metadata. The harness folds a stream into its
/// terminal `ModelResponse` before middleware regains control, so this one
/// typed boundary records primary and fallback routes identically without an
/// OpenHuman model wrapper or task-local propagation.
pub(super) struct ResolvedRouteMiddleware;

#[async_trait]
impl ModelMiddleware<(), crate::agent::tinyagents::host::OpenHumanRunContext>
    for ResolvedRouteMiddleware
{
    fn name(&self) -> &str {
        "openhuman.resolved_route"
    }

    async fn wrap_model(
        &self,
        ctx: &mut RunContext<crate::agent::tinyagents::host::OpenHumanRunContext>,
        state: &(),
        request: ModelRequest,
        next: ModelHandler<'_, (), crate::agent::tinyagents::host::OpenHumanRunContext>,
    ) -> tinyagents_harness::Result<MiddlewareModelOutcome> {
        let outcome = next.run(ctx, state, request).await?;
        let response = outcome.into_response();
        if let Some(route) = response.resolved_route.clone() {
            *ctx.data
                .resolved_route
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(route);
        }
        Ok(MiddlewareModelOutcome::from(response))
    }
}

#[cfg(test)]
#[path = "routes_tests.rs"]
mod tests;
