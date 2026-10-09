use super::*;
use crate::core::runtime::{context::CoreContext, ContextOverlay, DomainSet};
use crate::tools::toolpacks::ToolGroups;

fn agent(parent: &Arc<CoreContext>, id: &str) -> Arc<CoreContext> {
    parent.derive_with(
        ContextOverlay::new(
            crate::config::Config::default(),
            DomainSet::kernel(),
            ToolGroups::none(),
        )
        .session_agent(id),
    )
}

#[tokio::test]
async fn each_agent_context_owns_its_turn_tables() {
    let root = CoreContext::for_test(DomainSet::full(), None);
    let alpha = agent(&root, "alpha");
    let beta = agent(&root, "beta");

    let (alpha_sessions, alpha_in_flight, alpha_parallel) =
        CoreContext::scope(Arc::clone(&alpha), async {
            (thread_sessions(), in_flight(), parallel_in_flight())
        })
        .await;
    let (beta_sessions, beta_in_flight, beta_parallel) = CoreContext::scope(beta, async {
        (thread_sessions(), in_flight(), parallel_in_flight())
    })
    .await;
    let alpha_again = CoreContext::scope(alpha, async { in_flight() }).await;

    assert!(!Arc::ptr_eq(&alpha_sessions, &beta_sessions));
    assert!(!Arc::ptr_eq(&alpha_in_flight, &beta_in_flight));
    assert!(!Arc::ptr_eq(&alpha_parallel, &beta_parallel));
    assert!(Arc::ptr_eq(&alpha_in_flight, &alpha_again));
}
