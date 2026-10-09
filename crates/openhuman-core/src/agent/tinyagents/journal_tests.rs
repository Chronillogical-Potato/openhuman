use super::*;
use crate::core::runtime::agent_scope::test_agent_context;
use crate::core::runtime::{context::CoreContext, DomainSet};

#[tokio::test]
async fn request_journal_runs_stay_with_the_agent_that_registered_them() {
    let root = CoreContext::for_test(DomainSet::full(), None);
    let alpha = test_agent_context(&root, "alpha");
    let beta = test_agent_context(&root, "beta");

    CoreContext::scope(std::sync::Arc::clone(&alpha), async {
        register_request_journal_run("req-shared", "run-alpha");
    })
    .await;

    assert_eq!(
        CoreContext::scope(beta, async { take_request_journal_run("req-shared") }).await,
        None
    );
    assert_eq!(
        CoreContext::scope(alpha, async { take_request_journal_run("req-shared") }).await,
        Some("run-alpha".to_string())
    );
}
