use super::*;
use crate::config::{McpAuthConfig, McpServerConfig};
use serde_json::json;

fn name(tool: &str) -> String {
    tinymcp::tools::naming::disambiguated_tool_name("ticktick", "ticktick", tool)
}

const SECRET: &str = "sk-live-configured-secret-123";

struct Server;

impl wiremock::Respond for Server {
    fn respond(&self, request: &wiremock::Request) -> wiremock::ResponseTemplate {
        let body: Value = serde_json::from_slice(&request.body).unwrap_or_default();
        let result = match body["method"].as_str().unwrap_or_default() {
            "initialize" => json!({
                "protocolVersion": tinymcp_bus::LATEST_PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "goals", "version": "1.0.0" },
            }),
            "notifications/initialized" => return wiremock::ResponseTemplate::new(202),
            "tools/list" => json!({ "tools": [
                {
                    "name": "readGoals",
                    "description": "Read the goals on a list",
                    "inputSchema": { "type": "object", "properties": { "list": { "type": "string" } } },
                },
                { "name": "archive", "description": "Archive a goal" },
            ]}),
            "tools/call" if body["params"]["arguments"]["name"] == "fail" => {
                return wiremock::ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0",
                    "id": body["id"].clone(),
                    "error": { "code": -1, "message": format!("failed with {SECRET}") },
                }));
            }
            "tools/call" => json!({
                "content": [{ "type": "text", "text": format!("goals for {SECRET}") }],
            }),
            _ => json!({}),
        };
        wiremock::ResponseTemplate::new(200).set_body_json(json!({
            "jsonrpc": "2.0",
            "id": body["id"].clone(),
            "result": result,
        }))
    }
}

async fn server() -> wiremock::MockServer {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(Server)
        .mount(&server)
        .await;
    server
}

fn config(
    workspace: &std::path::Path,
    endpoint: &str,
    customize: impl FnOnce(&mut McpServerConfig),
) -> Config {
    let mut config = Config {
        workspace_dir: workspace.join("workspace"),
        action_dir: workspace.join("workspace"),
        config_path: workspace.join("config.toml"),
        ..Default::default()
    };
    config.gitbooks.enabled = false;
    let mut server = McpServerConfig {
        server: tinymcp_bus::McpServerConfig {
            name: "ticktick".into(),
            endpoint: endpoint.into(),
            auth: McpAuthConfig::BearerToken {
                token: SECRET.into(),
            },
            ..Default::default()
        },
        ..Default::default()
    };
    customize(&mut server);
    config.mcp_client.servers.push(server);
    config
}

async fn warmed(config: &Config) -> Arc<McpServerRegistry> {
    tokio::fs::create_dir_all(config.config_path.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(
        &config.config_path,
        toml::to_string(config).expect("serialize config"),
    )
    .await
    .unwrap();
    let registry = Arc::new(crate::mcp::host::static_registry(config));
    let host = crate::mcp::host::for_config(config).expect("host");
    for (name, outcome) in registry.refresh_tool_cache(host.dynamic().store()).await {
        outcome.unwrap_or_else(|error| panic!("{name}: {error}"));
    }
    registry
}

fn security() -> Arc<SecurityPolicy> {
    Arc::new(SecurityPolicy::default())
}

#[tokio::test]
async fn cached_tools_become_deferred_mcp_server_tool_names() {
    let mock = server().await;
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path(), &format!("{}/mcp", mock.uri()), |_| {});
    let registry = warmed(&config).await;

    let tools = configured_server_tools(&config, &registry, &security(), &HashSet::new());
    let names: Vec<&str> = tools.iter().map(|tool| tool.name()).collect();
    assert_eq!(names, [name("archive"), name("readGoals")]);
    assert!(names[1].starts_with("mcp_ticktick_read_goals_"));
    assert!(tools
        .iter()
        .all(|tool| tool.exposure() == ToolExposure::Deferred));
    assert_eq!(tools[1].family(), Some("ticktick"));
    assert_eq!(tools[1].permission_level(), PermissionLevel::Execute);
    assert!(tools[1].external_effect());
}

#[tokio::test]
async fn a_call_reaches_the_server_and_its_output_is_scrubbed() {
    let mock = server().await;
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path(), &format!("{}/mcp", mock.uri()), |_| {});
    let registry = warmed(&config).await;
    let tools = configured_server_tools(&config, &registry, &security(), &HashSet::new());

    let read = tools
        .iter()
        .find(|tool| tool.name() == name("readGoals"))
        .unwrap();
    let result = read.execute(json!({ "list": "work" })).await.unwrap();
    assert!(!result.is_error, "{}", result.text());
    assert_eq!(result.text(), "goals for [redacted]");
}

#[tokio::test]
async fn a_remote_call_error_is_scrubbed() {
    let mock = server().await;
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path(), &format!("{}/mcp", mock.uri()), |_| {});
    let registry = warmed(&config).await;
    let tools = configured_server_tools(&config, &registry, &security(), &HashSet::new());
    let read = tools
        .iter()
        .find(|tool| tool.name() == name("readGoals"))
        .unwrap();

    let result = read.execute(json!({ "name": "fail" })).await.unwrap();
    assert!(result.is_error);
    let text = result.text();
    assert!(text.contains("[redacted]"), "{text}");
    assert!(!text.contains(SECRET));
}

#[tokio::test]
async fn a_configured_tool_rejects_unreadable_current_configuration() {
    let mock = server().await;
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path(), &format!("{}/mcp", mock.uri()), |_| {});
    let registry = warmed(&config).await;
    let tools = configured_server_tools(&config, &registry, &security(), &HashSet::new());
    tokio::fs::remove_file(&config.config_path).await.unwrap();
    tokio::fs::create_dir(&config.config_path).await.unwrap();

    let read = tools
        .iter()
        .find(|tool| tool.name() == name("readGoals"))
        .unwrap();
    let error = read.execute(json!({})).await.unwrap_err();
    assert!(error
        .to_string()
        .contains("could not reload MCP configuration"));
    assert_eq!(tools_list_requests(&mock).await, 1);
}

#[tokio::test]
async fn act_policy_denial_prevents_configured_server_call() {
    let mock = server().await;
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path(), &format!("{}/mcp", mock.uri()), |_| {});
    let registry = warmed(&config).await;
    let denied = Arc::new(SecurityPolicy {
        enabled: true,
        autonomy: crate::security::AutonomyLevel::ReadOnly,
        ..SecurityPolicy::default()
    });
    let tools = configured_server_tools(&config, &registry, &denied, &HashSet::new());
    let read = tools
        .iter()
        .find(|tool| tool.name() == name("readGoals"))
        .unwrap();

    let error = read.execute(json!({ "list": "work" })).await.unwrap_err();
    assert!(error.to_string().contains("read-only mode"), "{error}");
    assert_eq!(tools_list_requests(&mock).await, 1);
    let calls = mock.received_requests().await.unwrap_or_default();
    assert!(!calls.iter().any(|request| {
        serde_json::from_slice::<Value>(&request.body)
            .map(|body| body["method"] == "tools/call")
            .unwrap_or(false)
    }));
}

#[tokio::test]
async fn expose_direct_and_direct_tools_are_honoured() {
    let mock = server().await;
    let dir = tempfile::tempdir().unwrap();
    let endpoint = format!("{}/mcp", mock.uri());

    let config_direct = config(dir.path(), &endpoint, |server| {
        server.expose = McpToolExposure::Direct;
    });
    let registry = warmed(&config_direct).await;
    let tools = configured_server_tools(&config_direct, &registry, &security(), &HashSet::new());
    assert!(tools
        .iter()
        .all(|tool| tool.exposure() == ToolExposure::Direct));

    let config_pinned = config(dir.path(), &endpoint, |server| {
        server.direct_tools = vec!["readGoals".into()];
    });
    let registry = warmed(&config_pinned).await;
    let tools = configured_server_tools(&config_pinned, &registry, &security(), &HashSet::new());
    let exposure = |remote: &str| {
        tools
            .iter()
            .find(|tool| tool.name() == name(remote))
            .unwrap()
            .exposure()
    };
    assert_eq!(exposure("readGoals"), ToolExposure::Direct);
    assert_eq!(exposure("archive"), ToolExposure::Deferred);
}

#[tokio::test]
async fn a_cold_cache_yields_no_tools_and_reserved_names_are_kept() {
    let mock = server().await;
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path(), &format!("{}/mcp", mock.uri()), |_| {});

    let cold = Arc::new(crate::mcp::host::static_registry(&config));
    assert!(configured_server_tools(&config, &cold, &security(), &HashSet::new()).is_empty());

    let registry = warmed(&config).await;
    let reserved: HashSet<String> = [name("archive")].into_iter().collect();
    let tools = configured_server_tools(&config, &registry, &security(), &reserved);
    let names: Vec<&str> = tools.iter().map(|tool| tool.name()).collect();
    assert_eq!(names, [name("readGoals")]);
}

async fn tools_list_requests(mock: &wiremock::MockServer) -> usize {
    mock.received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .filter(|request| {
            serde_json::from_slice::<Value>(&request.body)
                .map(|body| body["method"] == "tools/list")
                .unwrap_or(false)
        })
        .count()
}

#[tokio::test]
async fn the_app_load_refresh_fills_the_cache_and_builds_do_not_relist() {
    let mock = server().await;
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path(), &format!("{}/mcp", mock.uri()), |_| {});

    crate::mcp::refresh_configured_tool_cache(&config).await;
    assert_eq!(tools_list_requests(&mock).await, 1);

    // Later builds read the cache: no listing until the next app load or an
    // MCP change.
    for _ in 0..3 {
        let registry = Arc::new(crate::mcp::host::static_registry(&config));
        let tools = configured_server_tools(&config, &registry, &security(), &HashSet::new());
        assert_eq!(tools.len(), 2);
    }
    assert_eq!(tools_list_requests(&mock).await, 1);

    // An edited definition is an MCP change: its cache misses.
    let mut edited = config.clone();
    edited.mcp_client.servers[0].disallowed_tools = vec!["archive".into()];
    let registry = Arc::new(crate::mcp::host::static_registry(&edited));
    assert!(configured_server_tools(&edited, &registry, &security(), &HashSet::new()).is_empty());
}
