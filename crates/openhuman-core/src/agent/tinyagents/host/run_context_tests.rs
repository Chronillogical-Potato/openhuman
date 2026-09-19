use super::*;

#[test]
fn converts_explicit_values_to_the_canonical_tinyagents_context() {
    let cancellation = tinyagents_harness::cancel::CancellationToken::new();
    let workspace = tinytools::WorkspaceDescriptor::new("/work/action");
    let mut host = OpenHumanRunContext::new()
        .with_cancellation(cancellation.clone())
        .with_workspace(workspace.clone());
    host.thread_id = Some("thread-a".to_string());

    let run = host.into_tinyagents(tinyagents_harness::context::RunConfig::new("run-a"));

    assert_eq!(run.thread_id().map(|id| id.as_str()), Some("thread-a"));
    assert_eq!(run.workspace, Some(workspace));
    cancellation.cancel();
    assert!(run.cancellation.is_cancelled());
}

#[test]
fn child_inherits_tree_handles_but_isolates_observations_and_usage() {
    let mut parent = OpenHumanRunContext::new();
    parent.thread_id = Some("thread-a".to_string());
    parent.file_state_agent_id = Some("parent-file-state".to_string());
    *parent.resolved_route.lock().expect("route lock") =
        Some(crate::agent::tinyagents::ResolvedProviderRoute {
            provider: "provider-a".to_string(),
            model: "model-a".to_string(),
        });

    let child = parent.child();

    assert_eq!(child.spawn_depth, 1);
    assert_eq!(child.thread_id.as_deref(), Some("thread-a"));
    assert!(child.file_state_agent_id().is_none());
    assert!(child
        .resolved_route
        .lock()
        .expect("child route lock")
        .is_none());
    assert!(child
        .subagent_usage
        .lock()
        .expect("child usage lock")
        .is_empty());

    parent.cancellation.cancel();
    assert!(child.cancellation.is_cancelled());
}
