use super::*;

use crate::modules::security::gap::{install_budget_port, BudgetOp, ScriptedBudgetPort};
use tinysecurity_bus::BudgetDecision2;

#[tokio::test]
async fn action_spending_uses_tinysecurity_and_does_not_keep_a_host_counter() {
    let port = ScriptedBudgetPort::replying(BudgetDecision2::Allow);
    let _installed = install_budget_port(port.clone());
    let policy = SecurityPolicy {
        enabled: true,
        max_actions_per_hour: 9,
        ..SecurityPolicy::default()
    };

    policy.spend_action("file_write").await.unwrap();

    assert_eq!(policy.tracker.count(), 0);
    assert_eq!(
        port.asked(),
        vec![(BudgetOp::Spend, 9)],
        "action permission and accounting must come from the bus"
    );
}

#[tokio::test]
async fn a_spent_module_budget_is_reported_with_the_module_limit() {
    let port = ScriptedBudgetPort::replying(BudgetDecision2::RateLimited { max_per_hour: 4 });
    let _installed = install_budget_port(port);
    let policy = SecurityPolicy {
        enabled: true,
        max_actions_per_hour: 9,
        ..SecurityPolicy::default()
    };

    let error = policy.spend_action("file_write").await.unwrap_err();

    assert!(error.contains("4 actions/hour"), "{error}");
    assert!(error.contains("Rate limit exceeded"), "{error}");
}
