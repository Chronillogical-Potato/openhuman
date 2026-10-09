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
    assert!(!live_agents().contains(&"agents-test-live".to_string()));
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
    assert!(!live_agents().contains(&"agents-test-siblings".to_string()));
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
