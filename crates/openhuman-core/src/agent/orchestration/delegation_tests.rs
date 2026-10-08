use super::*;

fn root() -> RunContext<OpenHumanRunContext> {
    OpenHumanRunContext::new().into_tinyagents(RunConfig::new("delegation-root"))
}

#[test]
fn stage_host_data_carries_the_stage_runs_linked_token() {
    let graph_parent = root();
    let (stage_run, stage_context) = stage_run_contexts(&graph_parent, None).expect("stage");

    assert_eq!(stage_run.depth(), 1);
    // The graph cancelling reaches the stage's host carrier through the parent.
    graph_parent.cancellation.cancel();
    assert!(stage_run.cancellation.is_cancelled());
    assert!(stage_context.cancellation.is_cancelled());
    assert!(stage_run.data.cancellation.is_cancelled());
}

#[test]
fn cancelling_a_stage_spares_the_graph_and_its_siblings() {
    let graph_parent = root();
    let (stage_a, ctx_a) = stage_run_contexts(&graph_parent, None).expect("stage a");
    let (stage_b, ctx_b) = stage_run_contexts(&graph_parent, None).expect("stage b");

    stage_a.cancellation.cancel();
    assert!(ctx_a.cancellation.is_cancelled());
    assert!(stage_a.data.cancellation.is_cancelled());
    assert!(!stage_b.data.cancellation.is_cancelled());
    assert!(!graph_parent.cancellation.is_cancelled());
    assert!(!graph_parent.data.cancellation.is_cancelled());
    assert!(!stage_b.cancellation.is_cancelled());
    assert!(!ctx_b.cancellation.is_cancelled());
}
