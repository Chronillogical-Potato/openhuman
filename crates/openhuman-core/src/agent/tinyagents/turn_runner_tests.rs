use super::*;
use crate::agent::tinyagents::TurnModelSource;
use std::sync::Arc;
use tinyagents_harness::host::{ContextComposer, TurnContextRequest};

fn hosted_base() -> Arc<crate::agent::tinyagents::host::OpenHumanHostBase> {
    Arc::new(crate::agent::tinyagents::host::OpenHumanHostBase {
        config: Arc::new(crate::config::Config::default()),
        definitions: Arc::new(
            crate::agent::harness::definition::AgentDefinitionRegistry::builtins_only(),
        ),
        security_policy: Arc::new(crate::security::policy::SecurityPolicy::default()),
        memory: crate::memory::test_support::noop_memory(),
        post_turn_hooks: Vec::new(),
        session_definition: None,
    })
}

fn root_models(reply: &str) -> TurnModels {
    let model: Arc<dyn tinyinference_llm::model::ChatModel<()>> =
        Arc::new(tinyagents_harness::testkit::ScriptedModel::replies(vec![
            reply,
        ]));
    TurnModelSource::from_model(model)
        .build("root-test-model", 0.0, None, None)
        .expect("scripted turn models build")
}

fn root_messages(label: &str) -> Vec<ChatMessage> {
    vec![
        ChatMessage::system(format!("system-{label}")),
        ChatMessage::user(format!("user-{label}")),
    ]
}

fn root_context(
    thread_id: &str,
    workspace: &str,
    progress: tokio::sync::mpsc::Sender<crate::agent::progress::AgentProgress>,
) -> OpenHumanRunContext {
    let mut context = OpenHumanRunContext::new();
    context.origin = Some(crate::agent::turn_origin::AgentTurnOrigin::WebChat {
        thread_id: thread_id.to_string(),
        client_id: format!("client-{thread_id}"),
        request_id: Some(format!("request-{thread_id}")),
    });
    context.thread_id = Some(thread_id.to_string());
    context.workspace = Some(tinytools::WorkspaceDescriptor::new(workspace));
    context.progress = Some(progress);
    context
}

async fn run_root(
    base: Arc<crate::agent::tinyagents::host::OpenHumanHostBase>,
    context: OpenHumanRunContext,
    reply: &str,
) -> TinyagentsTurnOutcome {
    run_root_turn_via_hosted_agent(
        context,
        base,
        "main".to_string(),
        root_models(reply),
        "test".to_string(),
        "root-test-model",
        root_messages(reply),
        vec![Arc::new(Vec::new())],
        Some(Default::default()),
        2,
        None,
        None,
        &[],
        false,
        None,
        TurnContextMiddleware::default(),
        None,
        true,
    )
    .await
    .expect("hosted root succeeds")
}

#[tokio::test]
async fn precomposed_root_context_does_not_duplicate_the_session_prompt_or_preamble() {
    let request =
        TurnContextRequest::new("main", tinyagents_harness::ids::ThreadId::new("t"), "hi");

    assert_eq!(
        PrecomposedRootContext
            .compose_system_prompt(&request)
            .await
            .expect("precomposed root context composes"),
        "",
        "the frozen session system/context ladder stays in the invocation request"
    );
    assert!(
        PrecomposedRootContext
            .preamble(&request)
            .await
            .expect("precomposed root context builds preamble")
            .is_empty(),
        "host preparation must not insert a second root preamble"
    );
}

#[test]
fn hosted_roots_share_only_an_unconfigured_process_harness() {
    let first = root_hosted_harness() as *const _;
    let second = root_hosted_harness() as *const _;

    assert_eq!(first, second, "all roots enter the same durable harness");
    assert!(
        root_hosted_harness().models().default_name().is_none(),
        "models are invocation-local overlays, never mutable shared root state"
    );
    assert!(
        root_hosted_harness().tools().names().is_empty(),
        "tools are invocation-local overlays, never mutable shared root state"
    );
}

#[tokio::test]
async fn concurrent_hosted_roots_keep_models_progress_workspace_and_origin_isolated() {
    let base = hosted_base();
    let (left_progress, mut left_events) = tokio::sync::mpsc::channel(32);
    let (right_progress, mut right_events) = tokio::sync::mpsc::channel(32);

    let (left, right) = tokio::join!(
        run_root(
            base.clone(),
            root_context("left", "/tmp/left", left_progress),
            "left"
        ),
        run_root(
            base,
            root_context("right", "/tmp/right", right_progress),
            "right"
        ),
    );

    assert_eq!(left.text, "left");
    assert_eq!(right.text, "right");
    let left_history: Vec<_> = left
        .history
        .iter()
        .map(|message| (&message.role, &message.content))
        .collect();
    let right_history: Vec<_> = right
        .history
        .iter()
        .map(|message| (&message.role, &message.content))
        .collect();
    assert_ne!(
        left_history, right_history,
        "each overlay kept its transcript"
    );
    assert!(
        left_events.try_recv().is_ok(),
        "the left invocation retained its own progress sink"
    );
    assert!(
        right_events.try_recv().is_ok(),
        "the right invocation retained its own progress sink"
    );
}

#[tokio::test]
async fn a_streamed_delta_reaches_the_progress_channel_exactly_once() {
    let (progress, mut events) = tokio::sync::mpsc::channel(64);
    let outcome = run_root(
        hosted_base(),
        root_context("single", "/tmp/single", progress),
        "one delta",
    )
    .await;
    assert_eq!(outcome.text, "one delta");

    // `OpenhumanEventBridge` projects the crate's `ModelDelta` events onto the
    // channel; the host `ProgressSink` must not project the same tokens a
    // second time, or the web bridge interleaves two copies of every delta
    // ("TheThe resolver couldn't parse that exact phrase, so let resolver…").
    let mut streamed = Vec::new();
    let mut started = 0;
    let mut completed = 0;
    while let Ok(event) = events.try_recv() {
        match event {
            crate::agent::progress::AgentProgress::TextDelta { delta, .. } => streamed.push(delta),
            crate::agent::progress::AgentProgress::TurnStarted => started += 1,
            crate::agent::progress::AgentProgress::TurnCompleted { .. } => completed += 1,
            _ => {}
        }
    }
    assert_eq!(
        streamed,
        vec!["one delta".to_string()],
        "every model delta is forwarded once, by one producer"
    );
    // The two counters have OPPOSITE contracts. Read them separately.
    //
    // `completed` is asserted by equality at 0, and that is a documented
    // design: `run_root` passes `defer_turn_completed_to_caller = true`, so
    // `turn_runner` leaves `turn_completed_sink` as `None` and the seam emits no
    // terminal event — the caller emits the single one after its post-run
    // wrap-up. It was `<= 1` before, which a seam emitting once also satisfies,
    // and a seam emitting once on the deferring path is exactly the duplication
    // this test exists to prevent. So the ceiling could not fail in the
    // direction the test was written for. Do not loosen this one back.
    //
    // `started` deliberately stays a ceiling. The deferral flag governs
    // `TurnCompleted` only; it does not suppress the root's `Started` event.
    // `host/progress_sink.rs:400` forwards `TurnStarted` for the root run, and
    // the module docs at `:59` and `:185` say only the root projects a
    // top-level one — and this path IS the root, so exactly one is what the
    // contract calls for.
    //
    // It is nevertheless absent: instrumenting this test measured
    // `started = 0`. `assert_eq!(started, 1)` is therefore the RIGHT eventual
    // assertion and would fail today, so it is not made here — a PR tightening
    // a test should not land a knowingly-red one. Tracked as #6576; tighten
    // this to `== 1` as part of fixing that, not before.
    assert!(started <= 1, "TurnStarted was emitted {started} times");
    assert_eq!(
        completed, 0,
        "the deferring seam must emit no TurnCompleted — the caller owns it"
    );
}
