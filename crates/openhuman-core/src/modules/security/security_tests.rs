use super::*;

#[tokio::test]
async fn client_fails_closed_when_module_loading_is_disabled() {
    let mut config = Config::default();
    config.modules.enabled = false;

    assert!(matches!(
        client(&config).await,
        Err(SecurityError::Unavailable(_))
    ));
}

#[tokio::test]
#[ignore = "downloads and loads the pinned native TinySecurity release"]
async fn native_fixture_loads_the_pinned_release_and_reads_policy_info() {
    let mut config = Config::default();
    config.modules.enabled = true;

    let client = client(&config)
        .await
        .expect("pinned TinySecurity module should load on supported native runners");
    reinitialize(&config)
        .await
        .expect("the module should accept the same trusted host on reinitialization");
    let info = client
        .policy_info()
        .await
        .expect("the loaded module should answer its typed policy-info contract");

    assert!(super::policy::supports_contract(info.contract_version));
}
