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
        Some(tinyinference_llm::model::ResolvedModelRoute {
            provider: "provider-a".to_string(),
            model: "model-a".to_string(),
            route: "chat-v1".to_string(),
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

#[tokio::test]
async fn snapshots_scoped_stop_hooks_into_the_explicit_carrier() {
    let hooks: Vec<std::sync::Arc<dyn crate::agent::stop_hooks::StopHook>> =
        vec![std::sync::Arc::new(
            crate::agent::stop_hooks::MaxIterationsStopHook::new(3),
        )];

    crate::agent::stop_hooks::with_stop_hooks(hooks, async {
        let captured = OpenHumanRunContext::from_current_scopes();
        assert_eq!(captured.stop_hooks.len(), 1);
        assert_eq!(captured.stop_hooks[0].name(), "max_iterations");
    })
    .await;
}

#[tokio::test]
async fn snapshots_live_root_scopes_before_child_dispatch() {
    let root = tempfile::tempdir().expect("workspace");
    let (progress, _rx) = tokio::sync::mpsc::channel(1);

    crate::agent::turn_origin::with_origin(
        crate::agent::turn_origin::AgentTurnOrigin::Cli,
        crate::agent::turn_workspace::with_workspace(
            root.path().to_path_buf(),
            crate::agent::progress_sink::with_progress_sink(
                progress.clone(),
                crate::agent::tinyagents::thread_context::with_thread_id(
                    "thread-a",
                    crate::agent::harness::turn_dispatch_guard::with_dispatch_guard(None, async {
                        let expected_dispatch =
                            crate::agent::harness::turn_dispatch_guard::current()
                                .expect("dispatch scope");
                        let captured = OpenHumanRunContext::from_current_scopes();
                        assert!(matches!(
                            captured.origin,
                            Some(crate::agent::turn_origin::AgentTurnOrigin::Cli)
                        ));
                        assert!(captured.progress.is_some());
                        assert_eq!(captured.thread_id.as_deref(), Some("thread-a"));
                        assert_eq!(captured.workspace.expect("workspace").root, root.path());
                        assert!(std::sync::Arc::ptr_eq(
                            captured.dispatch.as_ref().expect("dispatch"),
                            &expected_dispatch,
                        ));
                        assert!(captured
                            .resolved_route
                            .lock()
                            .expect("route lock")
                            .is_none());
                    }),
                ),
            ),
        ),
    )
    .await;
}
