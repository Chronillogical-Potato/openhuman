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
    let runtime = crate::modules::runtime::runtime()
        .await
        .expect("module runtime remains available");
    eprintln!(
        "security module state after load: {:#?}",
        runtime.host().list()
    );
    let info = match client.policy_info().await {
        Ok(info) => info,
        Err(error) => {
            let record = crate::modules::registry::find(MODULE_ID).expect("registry record");
            let raw = runtime
                .connection()
                .proxy(record.bus_name, record.object_path, record.bus_name)
                .expect("module proxy")
                .call_confidential::<tinysecurity_bus::PolicyInfo>(
                    tinysecurity_bus::names::methods::POLICY_INFO,
                    (),
                )
                .await;
            panic!(
                "typed call failed ({error}); raw call: {raw:?}; state: {:#?}",
                runtime.host().list()
            );
        }
    };

    assert!(tinysecurity_bus::is_compatible(info.contract_version));
}
