use super::*;

#[test]
fn agentic_openrouter_uses_direct_billing() {
    let mut config = Config::default();
    config.agentic_provider = Some("openrouter:some/model".to_owned());
    assert_eq!(billing_route(&config), BillingRoute::DirectOpenRouter);
}

#[test]
fn other_agentic_routes_use_hosted_billing() {
    let mut config = Config::default();
    config.agentic_provider = Some("openhuman".to_owned());
    assert_eq!(billing_route(&config), BillingRoute::Hosted);
}
