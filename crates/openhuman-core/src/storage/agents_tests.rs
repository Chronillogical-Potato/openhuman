use super::*;
use crate::core::runtime::{ContextOverlay, DomainSet};

fn agent_context(agent: &str) -> Arc<CoreContext> {
    CoreContext::for_test(DomainSet::full(), None).derive_with(
        ContextOverlay::new(
            crate::config::Config::default(),
            DomainSet::full(),
            Default::default(),
        )
        .session_agent(agent),
    )
}

/// Whether `agent` stops being live. Another test's concurrent walk of the
/// registry can hold a context for an instant, so allow it to let go.
fn eventually_gone(agent: &str) -> bool {
    (0..100).any(|_| {
        let gone = !live_agents().contains(&agent.to_string());
        if !gone {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        gone
    })
}

fn live_agents() -> Vec<String> {
    agent_contexts(None).into_iter().map(|(id, _)| id).collect()
}

#[test]
fn deriving_an_agent_context_registers_it_until_dropped() {
    let context = agent_context("agents-test-live");
    assert!(live_agents().contains(&"agents-test-live".to_string()));
    let (_, found) = agent_contexts(None)
        .into_iter()
        .find(|(id, _)| id == "agents-test-live")
        .unwrap();
    assert!(Arc::ptr_eq(&found, &context), "the live context is used");
    drop((found, context));
    assert!(eventually_gone("agents-test-live"));
}

#[test]
fn a_context_without_an_agent_is_not_registered() {
    let before = live_agents().len();
    let _plain = CoreContext::for_test(DomainSet::full(), None);
    assert_eq!(live_agents().len(), before);
}

#[test]
fn for_agent_swaps_only_the_agent() {
    let parent = agent_context("agents-test-parent");
    let child = parent.for_agent("agents-test-other");
    assert_eq!(child.session_agent(), Some("agents-test-other"));
    assert_eq!(parent.session_agent(), Some("agents-test-parent"));
}

#[tokio::test]
async fn without_a_backend_only_the_local_scope_runs() {
    // The lib test binary never installs a backend into the process slot.
    if installed().is_some() {
        return;
    }
    let _agent = agent_context("agents-test-unvisited");
    let runs = for_each_scope("test", || async {
        CoreContext::current().and_then(|c| c.session_agent().map(str::to_string))
    })
    .await;
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].0, None);
    assert!(for_each_agent("test", || async {}).await.is_empty());
}

#[test]
fn a_dropped_sibling_context_does_not_hide_a_live_one() {
    let first = agent_context("agents-test-siblings");
    let second = agent_context("agents-test-siblings");
    drop(second);
    let (_, found) = agent_contexts(None)
        .into_iter()
        .find(|(id, _)| id == "agents-test-siblings")
        .expect("the first context is still live");
    assert!(Arc::ptr_eq(&found, &first));
    assert!(context_for("agents-test-siblings").is_some());
    drop((found, first));
    assert!(eventually_gone("agents-test-siblings"));
}

#[test]
fn reset_recorded_forgets_what_was_recorded() {
    RECORDED
        .lock()
        .unwrap()
        .insert("agents-test-recorded".into());
    reset_recorded();
    assert!(!RECORDED.lock().unwrap().contains("agents-test-recorded"));
}

fn memory_backend() -> Arc<dyn StorageBackend> {
    Arc::new(crate::storage::MemoryStorage::new())
}

#[test]
fn a_recorded_agent_is_visited_through_the_fallback_context() {
    let backend = memory_backend();
    reset_recorded();
    record_in(Arc::clone(&backend), "agents-test-recorded-only");
    assert!(recorded(Arc::clone(&backend)).contains(&"agents-test-recorded-only".to_string()));

    let fallback = CoreContext::for_test(DomainSet::full(), None);
    let visited = agent_contexts_in(Some(backend), Some(&fallback));
    let (_, context) = visited
        .iter()
        .find(|(id, _)| id == "agents-test-recorded-only")
        .expect("the recorded agent is visited");
    assert_eq!(context.session_agent(), Some("agents-test-recorded-only"));
}

#[test]
fn recording_is_skipped_for_an_agent_already_recorded_to_that_backend() {
    let first = memory_backend();
    reset_recorded();
    record_in(Arc::clone(&first), "agents-test-once");
    // A second backend is only populated once the cache is reset.
    let second = memory_backend();
    record_in(Arc::clone(&second), "agents-test-once");
    assert!(recorded(Arc::clone(&second)).is_empty());
    reset_recorded();
    record_in(Arc::clone(&second), "agents-test-once");
    assert_eq!(recorded(second), vec!["agents-test-once".to_string()]);
}

#[test]
fn without_a_fallback_only_live_contexts_are_visited() {
    let backend = memory_backend();
    reset_recorded();
    record_in(Arc::clone(&backend), "agents-test-no-fallback");
    assert!(!agent_contexts_in(Some(backend), None)
        .iter()
        .any(|(id, _)| id == "agents-test-no-fallback"));
}

#[test]
fn registering_forgets_agents_whose_contexts_are_all_gone() {
    drop(agent_context("agents-test-gone"));
    let _other = agent_context("agents-test-other-live");
    assert!(!LIVE.lock().unwrap().contains_key("agents-test-gone"));
}
