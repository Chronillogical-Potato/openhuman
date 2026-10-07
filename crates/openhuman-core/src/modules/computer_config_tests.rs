//! The module configuration follows `[computer]` and the stored credentials.

use super::*;
use crate::security::credentials::{api_key, AuthService};

fn config_in(dir: &std::path::Path) -> Config {
    let mut config = Config::default();
    config.config_path = dir.join("config.toml");
    config.workspace_dir = dir.join("workspace");
    config.secrets.encrypt = false;
    config
}

fn store(config: &Config, slug: &str, key: &str) {
    AuthService::from_config(config)
        .store_provider_token(
            &format!("provider:{slug}"),
            "default",
            key,
            Default::default(),
            true,
        )
        .expect("store provider key");
}

#[test]
fn signed_in_host_routes_jev_and_planner_through_tinyhumans() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = config_in(dir.path());
    config.computer.rescue_model = Some("openai/gpt-6-luna-pro".into());
    config.computer.planner_model = Some("anthropic/claude-sonnet-5".into());
    api_key::store_api_key(&config, "th_test_backend").unwrap();
    let value = module_config(&config);
    assert_eq!(value["jev"]["provider"], "tiny_humans_open_router");
    assert_eq!(value["planner"]["provider"], "tiny_humans");
    assert_eq!(value["planner"]["api_key"], "th_test_backend");
    assert!(value["planner"]["sdk_name"].is_string());
    assert_eq!(value["planner"]["rescue_model"], "openai/gpt-6-luna-pro");
    assert_eq!(value["planner"]["model"], "anthropic/claude-sonnet-5");
    assert_eq!(billing_route(&config), "hosted");
}

#[test]
fn byok_host_uses_openrouter_for_both() {
    let dir = tempfile::tempdir().unwrap();
    let config = config_in(dir.path());
    store(&config, "openrouter", "or_test_direct");
    let value = module_config(&config);
    assert_eq!(value["jev"]["provider"], "open_router");
    assert_eq!(value["planner"]["provider"], "open_router");
    assert_eq!(value["planner"]["api_key"], "or_test_direct");
    assert!(value["planner"].get("rescue_model").is_none());
    assert_eq!(billing_route(&config), "direct_openrouter");
}

#[test]
fn open_jev_and_sage_use_their_own_keys() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = config_in(dir.path());
    store(&config, "openjev", "oj_test");
    store(&config, "sage", "sage_test");

    config.computer.decision_model = DecisionModel::OpenJev;
    let value = module_config(&config);
    assert_eq!(
        value["jev"],
        json!({"api_key": "oj_test", "provider": "open_jev"})
    );
    assert_eq!(billing_route(&config), "open_jev");

    config.computer.decision_model = DecisionModel::Sage;
    config.computer.sage_fast = true;
    let value = module_config(&config);
    assert_eq!(
        value["jev"],
        json!({"api_key": "sage_test", "provider": "sage", "fast": true})
    );
    assert_eq!(billing_route(&config), "sage");
}

#[test]
fn chrome_path_is_the_module_browser_executable() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = config_in(dir.path());
    config.browser.chrome_path = Some("/Applications/Chrome.app".into());
    let value = module_config(&config);
    assert_eq!(
        value["browser"],
        json!({"executable": "/Applications/Chrome.app"})
    );
}
