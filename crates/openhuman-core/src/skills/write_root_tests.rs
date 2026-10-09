use super::*;
use crate::core::runtime::{context::CoreContext, ContextOverlay, DomainSet};
use crate::skills::ops_create::{create_workflow_inner, CreateWorkflowParams};
use crate::skills::ops_discover::discover_workflows;
use crate::skills::ops_types::WorkflowScope;
use crate::tools::toolpacks::ToolGroups;
use std::sync::Arc;

fn agent_on(workspace: &Path, agent: &str) -> Arc<CoreContext> {
    let mut config = crate::config::Config::default();
    config.workspace_dir = workspace.to_path_buf();
    CoreContext::for_test_with_config(DomainSet::full(), config.clone()).derive_with(
        ContextOverlay::new(config, DomainSet::full(), ToolGroups::none()).session_agent(agent),
    )
}

fn user_workflow(name: &str) -> CreateWorkflowParams {
    CreateWorkflowParams {
        name: name.to_string(),
        description: "An agent-owned workflow.".to_string(),
        scope: WorkflowScope::User,
        ..CreateWorkflowParams::default()
    }
}

fn names(workflows: &[crate::skills::Workflow]) -> Vec<String> {
    workflows
        .iter()
        .map(|workflow| workflow.name.clone())
        .collect()
}

async fn seen_by(home: &Path, workspace: &Path, agent: Option<&str>) -> Vec<String> {
    let discover = async { names(&discover_workflows(Some(home), Some(workspace), false)) };
    match agent {
        Some(agent) => CoreContext::scope(agent_on(workspace, agent), discover).await,
        None => discover.await,
    }
}

#[test]
fn outside_an_agent_the_operator_home_is_the_write_root() {
    let home = Path::new("/home/operator");
    let workspace = Path::new("/ws");
    assert_eq!(agent_skill_home(workspace), None);
    assert_eq!(
        user_skill_install_root(workspace, Some(home)),
        Some(home.join(".openhuman/skills"))
    );
    assert_eq!(
        user_workflow_root(workspace, Some(home)),
        Some(home.join(".openhuman/workflows"))
    );
}

#[tokio::test]
async fn an_agents_created_skill_lands_in_its_directory_and_only_it_discovers_it() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");
    let home_path = home.path().to_path_buf();
    let workspace_path = workspace.path().to_path_buf();

    let created = CoreContext::scope(agent_on(workspace.path(), "alpha"), {
        let (home_path, workspace_path) = (home_path.clone(), workspace_path.clone());
        async move {
            create_workflow_inner(
                Some(&home_path),
                &workspace_path,
                user_workflow("Alpha Owned"),
            )
            .expect("alpha creates a workflow")
        }
    })
    .await;

    assert!(created
        .location
        .as_ref()
        .is_some_and(|path| path.starts_with(workspace.path().join("agents/alpha/workflows"))));
    assert!(
        !home.path().join(".openhuman").exists(),
        "home stays untouched"
    );

    assert!(seen_by(&home_path, &workspace_path, Some("alpha"))
        .await
        .contains(&"alpha-owned".to_string()));
    assert!(!seen_by(&home_path, &workspace_path, Some("beta"))
        .await
        .contains(&"alpha-owned".to_string()));
    assert!(!seen_by(&home_path, &workspace_path, None)
        .await
        .contains(&"alpha-owned".to_string()));
}
