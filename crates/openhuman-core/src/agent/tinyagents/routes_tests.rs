use super::*;
use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use tinyagents_harness::agent_loop::AgentStreamItem;
use tinyagents_harness::context::RunConfig;
use tinyagents_harness::runtime::{AgentHarness, RunPolicy};
use tinyagents_harness::testkit::ScriptedModel;
use tinyinference_llm::message::Message;
use tinyinference_llm::model::{
    ChatModel, ModelRequest, ModelResponse, ResolvedModelRoute, RouteRecordingModel,
};

struct FailingModel;

#[async_trait]
impl ChatModel<()> for FailingModel {
    async fn invoke(
        &self,
        _state: &(),
        _request: ModelRequest,
    ) -> tinyinference_llm::Result<ModelResponse> {
        Err(tinyinference_llm::Error::Model(
            "primary unavailable".to_string(),
        ))
    }
}

async fn run_recorded_route(
    streaming: bool,
    fallback: bool,
    route: ResolvedModelRoute,
) -> ResolvedModelRoute {
    let mut harness: AgentHarness<(), crate::agent::tinyagents::host::OpenHumanRunContext> =
        AgentHarness::new();
    let served_route = if fallback {
        let primary = Arc::new(RouteRecordingModel::new(
            Arc::new(FailingModel),
            ResolvedModelRoute::new("primary", "unavailable", "primary"),
        ));
        harness
            .register_model("primary", primary)
            .set_default_model("primary");
        route.clone()
    } else {
        route.clone()
    };
    let successful = Arc::new(RouteRecordingModel::new(
        Arc::new(ScriptedModel::replies(vec!["done"])),
        served_route.clone(),
    ));
    if fallback {
        harness
            .register_model("backup", successful)
            .with_policy(RunPolicy {
                retry: tinyagents_harness::retry::RetryPolicy::default().with_max_attempts(1),
                fallback: Some(tinyagents_harness::retry::FallbackPolicy::new([
                    "primary", "backup",
                ])),
                ..RunPolicy::default()
            });
    } else {
        harness
            .register_model("primary", successful)
            .set_default_model("primary");
    }
    harness.push_model_middleware(Arc::new(ResolvedRouteMiddleware));

    let host = crate::agent::tinyagents::host::OpenHumanRunContext::new();
    let slot = host.resolved_route.clone();
    let ctx = host.into_tinyagents(RunConfig::new("route-recording"));
    if streaming {
        let mut stream =
            Box::pin(harness.invoke_stream_in_context(&(), ctx, vec![Message::user("route")]));
        while let Some(item) = stream.next().await {
            match item {
                AgentStreamItem::Completed(_) => break,
                AgentStreamItem::Failed { error, .. } => panic!("stream failed: {error}"),
                AgentStreamItem::Event(_) => {}
            }
        }
    } else {
        harness
            .invoke_in_context(&(), ctx, vec![Message::user("route")])
            .await
            .expect("unary route run succeeds");
    }
    slot.lock()
        .expect("route slot")
        .clone()
        .expect("middleware records canonical response route")
}

#[tokio::test]
async fn resolved_route_middleware_records_primary_routes_for_unary_and_streaming_runs() {
    for streaming in [false, true] {
        let route = ResolvedModelRoute::new("openhuman", "chat-concrete", "chat-v1");
        assert_eq!(
            run_recorded_route(streaming, false, route.clone()).await,
            route,
            "streaming={streaming}"
        );
    }
}

#[tokio::test]
async fn resolved_route_middleware_records_successful_fallback_for_unary_and_streaming_runs() {
    for streaming in [false, true] {
        let route = ResolvedModelRoute::new("anthropic", "claude-concrete", "backup");
        assert_eq!(
            run_recorded_route(streaming, true, route.clone()).await,
            route,
            "streaming={streaming}"
        );
    }
}

#[tokio::test]
async fn resolved_route_middleware_isolates_concurrent_run_contexts() {
    let first = ResolvedModelRoute::new("openai", "first", "chat-v1");
    let second = ResolvedModelRoute::new("anthropic", "second", "reasoning-v1");
    let (first_observed, second_observed) = tokio::join!(
        run_recorded_route(false, false, first.clone()),
        run_recorded_route(true, false, second.clone()),
    );
    assert_eq!(first_observed, first);
    assert_eq!(second_observed, second);
}

/// The fallback chain for every tier must lead with the primary and carry the
/// single same-family alternate the legacy static table encoded — the crate
/// `ModelRouter` projection is exactly behavior-neutral.
#[test]
fn route_fallback_policy_matches_legacy_chains() {
    let cases: &[(&str, Option<&[&str]>)] = &[
        (MODEL_CHAT_V1, Some(&[MODEL_CHAT_V1, MODEL_BURST_V1])),
        (MODEL_BURST_V1, Some(&[MODEL_BURST_V1, MODEL_CHAT_V1])),
        (
            MODEL_REASONING_V1,
            Some(&[MODEL_REASONING_V1, MODEL_AGENTIC_V1]),
        ),
        (
            MODEL_AGENTIC_V1,
            Some(&[MODEL_AGENTIC_V1, MODEL_REASONING_V1]),
        ),
        (MODEL_CODING_V1, Some(&[MODEL_CODING_V1, MODEL_AGENTIC_V1])),
        (
            MODEL_SUMMARIZATION_V1,
            Some(&[MODEL_SUMMARIZATION_V1, MODEL_CHAT_V1]),
        ),
        // Vision is primary-only (an image_in gate no text tier can satisfy).
        (MODEL_VISION_V1, None),
        ("hint:vision", None),
        // A raw non-tier model installs no chain.
        ("gpt-4o", None),
    ];
    for (model, expected) in cases {
        let got = route_fallback_policy(model).map(|p| p.models);
        let want = expected.map(|chain| chain.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(got, want, "fallback chain mismatch for {model}");
    }
}

/// Only the vision tier (and its hint form) imposes an `image_in` gate; the
/// common text turn stays ungated.
#[test]
fn turn_required_capabilities_gates_only_vision() {
    let vision = turn_required_capabilities(MODEL_VISION_V1).expect("vision is gated");
    assert!(vision.image_in);
    let hint = turn_required_capabilities("hint:vision").expect("hint:vision is gated");
    assert!(hint.image_in);
    for model in [
        MODEL_CHAT_V1,
        MODEL_REASONING_V1,
        MODEL_AGENTIC_V1,
        MODEL_CODING_V1,
        MODEL_BURST_V1,
        MODEL_SUMMARIZATION_V1,
        "gpt-4o",
    ] {
        assert!(
            turn_required_capabilities(model).is_none(),
            "{model} must not be capability-gated"
        );
    }
}

/// The router covers exactly the projected tier inventory (plus the hint:vision
/// gate alias), so the fallback/capability source of truth stays aligned with
/// `WORKLOAD_ROUTE_TIERS`.
#[test]
fn router_covers_the_workload_tier_inventory() {
    for tier in WORKLOAD_ROUTE_TIERS {
        assert!(
            OH_WORKLOAD_ROUTER.route(tier).is_some(),
            "router missing tier {tier}"
        );
    }
}
