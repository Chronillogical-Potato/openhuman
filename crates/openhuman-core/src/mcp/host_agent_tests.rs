use super::*;
use crate::core::runtime::{context::CoreContext, ContextOverlay, DomainSet};
use crate::tools::toolpacks::ToolGroups;

fn workspace_config(workspace: &Path) -> Config {
    let mut config = Config::default();
    config.gitbooks.enabled = false;
    config.workspace_dir = workspace.to_path_buf();
    config
}

fn agent_context(config: &Config, agent: &str) -> Arc<CoreContext> {
    CoreContext::for_test_with_config(DomainSet::full(), config.clone()).derive_with(
        ContextOverlay::new(config.clone(), DomainSet::full(), ToolGroups::none())
            .session_agent(agent),
    )
}

#[tokio::test]
async fn each_agent_gets_its_own_host_under_its_scope_dir() {
    let temporary = tempfile::tempdir().expect("tempdir");
    let config = workspace_config(temporary.path());

    let alpha = CoreContext::scope(agent_context(&config, "alpha"), async {
        for_config(&config).expect("alpha's host opens")
    })
    .await;
    let beta = CoreContext::scope(agent_context(&config, "beta"), async {
        for_config(&config).expect("beta's host opens")
    })
    .await;
    let workspace = for_config(&config).expect("the workspace host opens");

    assert!(!Arc::ptr_eq(&alpha, &beta));
    assert!(!Arc::ptr_eq(&alpha, &workspace));
    let alpha_dir = temporary.path().join("agents").join("alpha");
    assert!(tinymcp::Store::path_for(&alpha_dir).exists());
    assert!(tinymcp::AuditStore::path_for(&alpha_dir).exists());
    assert!(tinymcp::Store::path_for(temporary.path()).exists());
}

#[tokio::test]
async fn the_ambient_service_under_an_agent_is_that_agents_host() {
    let temporary = tempfile::tempdir().expect("tempdir");
    let config = workspace_config(temporary.path());

    let (ambient, addressed) = CoreContext::scope(agent_context(&config, "gamma"), async {
        (
            try_service().expect("an agent always has a host"),
            for_config(&config).expect("the agent's host opens"),
        )
    })
    .await;

    assert!(Arc::ptr_eq(&ambient, &addressed));
    let outside = for_config(&config).expect("the workspace host opens");
    assert!(!Arc::ptr_eq(&ambient, &outside));
}
