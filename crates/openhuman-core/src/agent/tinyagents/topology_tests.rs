use super::*;

#[test]
fn all_topologies_includes_member_delegation_and_workflow_scheduler() {
    let reports = all_graph_topologies();
    for name in [
        "agent_teams:member",
        "delegation",
        "workflow_runs:scheduler_preview",
    ] {
        let report = reports
            .iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("the {name} graph should be exported"));
        assert!(
            report.ok,
            "{name} graph should validate structurally: {:?}",
            report.errors
        );
        assert!(
            report.mermaid.contains("flowchart"),
            "{name} mermaid should render: {}",
            report.mermaid
        );
    }
}

#[test]
fn all_topologies_names_do_not_collide_with_production_tools() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let config = std::sync::Arc::new(crate::config::Config::default());
    let security = std::sync::Arc::new(crate::security::SecurityPolicy::default());
    let audit = std::sync::Arc::new(
        crate::security::AuditLogger::new(
            crate::config::AuditConfig {
                enabled: false,
                log_path: "audit.log".into(),
                max_size_mb: 10,
            },
            tmp.path().to_path_buf(),
        )
        .expect("create audit logger"),
    );
    let browser = crate::config::BrowserConfig::default();
    let http = crate::config::HttpRequestConfig::default();
    let agents = std::collections::HashMap::new();

    let tools = crate::tools::ops::all_tools(
        config.clone(),
        &security,
        audit,
        &browser,
        &http,
        tmp.path(),
        &agents,
        &config,
    );
    let tool_names: std::collections::HashSet<_> = tools.iter().map(|t| t.name()).collect();

    // Verify runtime tool `spawn_parallel_agents` is present in the checked tool set
    assert!(
        tool_names.contains("spawn_parallel_agents"),
        "expected spawn_parallel_agents in production all_tools registration"
    );

    let reports = all_graph_topologies();
    for report in &reports {
        assert!(
            !tool_names.contains(report.name),
            "graph name '{}' collides with registered tool of the same name",
            report.name
        );
    }
}
