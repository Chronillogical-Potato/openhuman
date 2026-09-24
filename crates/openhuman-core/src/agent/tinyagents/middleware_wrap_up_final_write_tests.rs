//! The penultimate call of a capped turn: `reserve_final_write`.
//!
//! The conclusion clears the belt so the last call cannot be spent on another
//! tool. That guarantees an answer — and, for a turn whose deliverable is a
//! *file*, guarantees losing it: the model arrives holding the finished content
//! with nowhere to put it. These cover the call before it, where the belt is
//! narrowed to the writers instead.

use super::*;

/// The sink, in the shape `FinalCallWrapUpMiddleware` takes it.
fn sink_with(entries: &[(&str, &str)]) -> crate::agent::tinyagents::ToolOutcomeSink {
    std::sync::Arc::new(std::sync::Mutex::new(
        entries
            .iter()
            .map(|(id, content)| crate::agent::tinyagents::ToolCallOutcome {
                call_id: (*id).to_string(),
                name: "fetch".to_string(),
                arguments: serde_json::Value::Null,
                success: true,
                content: (*content).to_string(),
                duration_ms: 0,
            })
            .collect(),
    ))
}

fn mw(sink: crate::agent::tinyagents::ToolOutcomeSink) -> FinalCallWrapUpMiddleware {
    FinalCallWrapUpMiddleware::new("CONCLUDE NOW", "WRITE NOW", sink, 0)
}

/// A context sitting on the Nth call of an N-call budget, minus `back`.
fn ctx_at(
    max: usize,
    back: usize,
) -> RunContext<crate::agent::tinyagents::host::OpenHumanRunContext> {
    let mut ctx = RunContext::new(
        RunConfig::new("mw-test").with_max_model_calls(max),
        crate::agent::tinyagents::host::OpenHumanRunContext::new(),
    );
    for _ in 0..(max - back) {
        ctx.limits.record_model_call().unwrap();
    }
    ctx
}

/// A mixed belt: two gatherers and the two writers.
fn mixed_belt() -> Vec<ToolSchema> {
    ["web_fetch", "file_write", "shell", "apply_patch"]
        .iter()
        .map(|n| ToolSchema::new(*n, *n, serde_json::json!({})))
        .collect()
}

fn names(request: &ModelRequest) -> Vec<String> {
    request.tools.iter().map(|t| t.name.clone()).collect()
}

/// The defect this exists for: on the call before the conclusion the writers
/// survive, so a turn that owes a file can still write it.
#[tokio::test]
async fn penultimate_call_keeps_the_writers_and_drops_the_gatherers() {
    let mw = mw(sink_with(&[]));
    let mut ctx = ctx_at(15, 1);
    let mut request = ModelRequest {
        messages: vec![TaMessage::user("summarise the baggage rules into a file")],
        tools: mixed_belt(),
        ..Default::default()
    };

    mw.before_model(&mut ctx, &(), &mut request).await.unwrap();

    assert_eq!(
        names(&request),
        vec!["file_write", "apply_patch"],
        "only the tools that can emit a deliverable may survive"
    );
    assert_eq!(
        request.messages.last().map(|m| m.text()),
        Some("WRITE NOW".to_string()),
        "the write instruction must be the final turn of the request"
    );
    assert!(
        !mw.fired().load(std::sync::atomic::Ordering::SeqCst),
        "this is not the conclusion: the turn has one more call and must not \
         be reported as capped yet"
    );
}

/// `Auto`, never `Required`. A turn that has already written its file, or was
/// only ever asked for an answer, must be free to spend this call on text —
/// forcing a call would make it invent a write.
#[tokio::test]
async fn penultimate_call_leaves_the_model_free_not_to_write() {
    let mw = mw(sink_with(&[]));
    let mut ctx = ctx_at(15, 1);
    let mut request = ModelRequest {
        messages: vec![TaMessage::user("hi")],
        tools: mixed_belt(),
        tool_choice: tinyinference_llm::model::ToolChoice::Required,
        ..Default::default()
    };

    mw.before_model(&mut ctx, &(), &mut request).await.unwrap();

    assert!(matches!(
        request.tool_choice,
        tinyinference_llm::model::ToolChoice::Auto
    ));
}

/// Asked to write its findings into a file, this call has to be able to read
/// them — so it gets the same restoration the conclusion does.
#[tokio::test]
async fn penultimate_call_restores_what_microcompact_cleared() {
    let mw = mw(sink_with(&[("call-old", "carry-on max 22 x 14 x 9 in")]));
    let mut ctx = ctx_at(15, 1);
    let mut request = ModelRequest {
        messages: vec![
            TaMessage::tool("call-old", CLEARED_PLACEHOLDER),
            TaMessage::tool("call-new", "checked bag 50 lb"),
        ],
        tools: mixed_belt(),
        ..Default::default()
    };

    mw.before_model(&mut ctx, &(), &mut request).await.unwrap();

    let bodies: Vec<String> = request.messages.iter().map(|m| m.text()).collect();
    assert!(
        bodies.iter().any(|b| b.contains("22 x 14 x 9")),
        "a call asked to write findings down must be able to read them: {bodies:?}"
    );
}

/// A belt with no writer on it gets no narrowing and no instruction: telling a
/// read-only agent that "the only tools left are the ones that write files"
/// would simply be false.
#[tokio::test]
async fn a_belt_without_a_writer_is_left_alone() {
    let mw = mw(sink_with(&[]));
    let mut ctx = ctx_at(15, 1);
    let mut request = ModelRequest {
        messages: vec![TaMessage::user("hi")],
        tools: vec![
            ToolSchema::new("web_fetch", "web_fetch", serde_json::json!({})),
            ToolSchema::new("shell", "shell", serde_json::json!({})),
        ],
        ..Default::default()
    };

    mw.before_model(&mut ctx, &(), &mut request).await.unwrap();

    assert_eq!(names(&request), vec!["web_fetch", "shell"]);
    assert_eq!(
        request.messages.len(),
        1,
        "no instruction should be appended"
    );
}

/// Reserving out of a two-call budget would leave no round in which anything
/// could be gathered to write, so it is skipped: one tool round, then the
/// conclusion, exactly as before.
#[tokio::test]
async fn a_two_call_budget_keeps_its_one_gathering_round() {
    let mw = mw(sink_with(&[]));
    let mut ctx = ctx_at(2, 1);
    let mut request = ModelRequest {
        messages: vec![TaMessage::user("hi")],
        tools: mixed_belt(),
        ..Default::default()
    };

    mw.before_model(&mut ctx, &(), &mut request).await.unwrap();

    assert_eq!(
        names(&request),
        vec!["web_fetch", "file_write", "shell", "apply_patch"],
        "the belt must stay whole when there is no room to reserve a call"
    );
    assert_eq!(
        request.messages.len(),
        1,
        "no instruction should be appended"
    );
}

/// Two calls earlier nothing has happened yet — the reservation is for the
/// penultimate call alone, not for the tail of the turn.
#[tokio::test]
async fn the_call_before_the_penultimate_one_is_untouched() {
    let mw = mw(sink_with(&[]));
    let mut ctx = ctx_at(15, 2);
    let mut request = ModelRequest {
        messages: vec![TaMessage::user("hi")],
        tools: mixed_belt(),
        ..Default::default()
    };

    mw.before_model(&mut ctx, &(), &mut request).await.unwrap();

    assert_eq!(request.tools.len(), 4, "the belt must stay intact mid-turn");
    assert_eq!(request.messages.len(), 1);
}

/// And the call after it still clears the belt outright: narrowing buys the
/// artifact, the conclusion still has to be text.
#[tokio::test]
async fn the_conclusion_still_withdraws_everything() {
    let mw = mw(sink_with(&[]));
    let mut ctx = ctx_at(15, 0);
    let mut request = ModelRequest {
        messages: vec![TaMessage::user("hi")],
        tools: mixed_belt(),
        ..Default::default()
    };

    mw.before_model(&mut ctx, &(), &mut request).await.unwrap();

    assert!(
        request.tools.is_empty(),
        "the final call keeps nothing, writers included"
    );
    assert_eq!(
        request.messages.last().map(|m| m.text()),
        Some("CONCLUDE NOW".to_string())
    );
    assert!(mw.fired().load(std::sync::atomic::Ordering::SeqCst));
}
