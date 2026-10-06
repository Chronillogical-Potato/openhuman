//! Tests for hosting account resolution and the tool set it exposes.
//!
//! The provider and the tools themselves (workspace containment, argument
//! validation, the rollback guard, the declarations a model sees) are TinyHosts'
//! and are tested there. What is tested here is the seam: whether an account
//! resolves from configuration and hands the crate's tools its host and workspace.

use super::*;
use crate::config::Config;

fn config_with(workspace: &std::path::Path, enabled: bool, api_key: &str) -> Config {
    let mut config = Config::default();
    config.workspace_dir = workspace.to_path_buf();
    config.hosting.enabled = enabled;
    config.hosting.api_key = api_key.to_string();
    config
}

#[test]
fn hosting_off_yields_no_account() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let config = config_with(workspace.path(), false, "token");

    assert!(Account::from_config(&config)
        .expect("resolution does not fail")
        .is_none());
}

#[test]
fn a_configured_key_yields_an_account() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let config = config_with(workspace.path(), true, "token");

    let account = Account::from_config(&config)
        .expect("resolution does not fail")
        .expect("an account, since a key is configured");

    assert_eq!(account.host().kind().as_str(), "vercel");
    assert_eq!(account.workspace_dir(), workspace.path());
}

#[test]
fn an_unknown_provider_is_an_error_rather_than_a_silent_skip() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let mut config = config_with(workspace.path(), true, "token");
    config.hosting.provider = "heroku".to_string();

    let error = Account::from_config(&config).expect_err("an unknown provider fails");

    assert!(
        error.to_string().contains("heroku"),
        "the error should name the provider: {error}"
    );
}

#[test]
fn an_account_reports_itself_without_its_credential() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let config = config_with(workspace.path(), true, "super-secret");

    let account = Account::from_config(&config)
        .expect("resolution does not fail")
        .expect("an account");

    assert!(
        !format!("{account:?}").contains("super-secret"),
        "the credential must never be rendered"
    );
}

#[test]
fn an_account_exposes_every_hosting_tool() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let config = config_with(workspace.path(), true, "token");
    let account = Account::from_config(&config)
        .expect("resolution does not fail")
        .expect("an account");

    let names: Vec<String> = account
        .tools()
        .iter()
        .map(|tool| tool.name().to_string())
        .collect();

    assert_eq!(
        names,
        [
            "hosting_launch_site",
            "hosting_deployment_status",
            "hosting_list_deployments",
            "hosting_deployment_logs",
            "hosting_rollback",
            "hosting_list_sites",
            "hosting_set_env",
            "hosting_add_domain",
            "hosting_domain_status",
            "hosting_analytics",
        ]
    );
}

#[test]
fn only_the_tools_that_change_the_world_carry_an_external_effect() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let config = config_with(workspace.path(), true, "token");
    let account = Account::from_config(&config)
        .expect("resolution does not fail")
        .expect("an account");

    for tool in account.tools() {
        let expected = matches!(
            tool.name(),
            "hosting_launch_site" | "hosting_set_env" | "hosting_add_domain" | "hosting_rollback"
        );
        assert_eq!(
            tool.external_effect(),
            expected,
            "{} has the wrong external effect",
            tool.name()
        );
    }
}
