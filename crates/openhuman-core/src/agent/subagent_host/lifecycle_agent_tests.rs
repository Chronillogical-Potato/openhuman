use super::*;
use crate::core::runtime::agent_scope::test_agent_context;
use crate::core::runtime::{context::CoreContext, DomainSet};

#[tokio::test]
async fn each_agent_dedupes_its_own_in_flight_sub_agents() {
    let root = CoreContext::for_test(DomainSet::full(), None);
    let alpha = test_agent_context(&root, "alpha");
    let beta = test_agent_context(&root, "beta");

    let alpha_map = CoreContext::scope(Arc::clone(&alpha), async { host_in_flight() }).await;
    let beta_map = CoreContext::scope(beta, async { host_in_flight() }).await;
    let alpha_again = CoreContext::scope(alpha, async { host_in_flight() }).await;

    assert!(!Arc::ptr_eq(&alpha_map, &beta_map));
    assert!(Arc::ptr_eq(&alpha_map, &alpha_again));
}
