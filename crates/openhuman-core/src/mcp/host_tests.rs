//! Unit tests for the configuration conversion.
//!
//! This is the one place two vocabularies meet, and a field dropped in the
//! conversion is silent: the server simply behaves as though the user never set
//! it. Each field is checked individually for that reason.

use super::*;
use crate::config::{Config, HttpHeader, McpAuthConfig, McpServerConfig as HostServer};
use std::collections::HashMap;

/// A configuration with the documentation server turned off, so the tests that
/// count servers are not counting it.
fn config_without_docs() -> Config {
    let mut config = Config::default();
    config.gitbooks.enabled = false;
    config
}

/// A declared server with every field set to something distinguishable.
fn populated_server() -> HostServer {
    HostServer {
        server: tinymcp_bus::McpServerConfig {
            name: "weather".into(),
            endpoint: "https://example.test/mcp".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "weather-mcp".into()],
            env: [("API_KEY".to_string(), "secret".to_string())].into(),
            cwd: Some("/tmp".into()),
            description: Some("Weather lookups".into()),
            enabled: false,
            allowed_tools: vec!["forecast".into()],
            disallowed_tools: vec!["debug".into()],
            timeout_secs: 9,
            auth: McpAuthConfig::BearerToken { token: "t".into() },
        },
        expose: Default::default(),
        direct_tools: Vec::new(),
    }
}

#[test]
fn every_field_of_a_declared_server_survives_the_conversion() {
    let mut config = config_without_docs();
    config.mcp_client.servers.push(populated_server());

    let converted = client_config(&config);
    let server = converted.servers.first().expect("the server");

    assert_eq!(server.name, "weather");
    assert_eq!(server.endpoint, "https://example.test/mcp");
    assert_eq!(server.command, "npx");
    assert_eq!(server.args, ["-y", "weather-mcp"]);
    assert_eq!(
        server.env.get("API_KEY").map(String::as_str),
        Some("secret")
    );
    assert_eq!(server.cwd.as_deref(), Some("/tmp"));
    assert_eq!(server.description.as_deref(), Some("Weather lookups"));
    assert!(!server.enabled);
    assert_eq!(server.allowed_tools, ["forecast"]);
    assert_eq!(server.disallowed_tools, ["debug"]);
    assert_eq!(server.timeout_secs, 9);
    assert_eq!(
        server.auth,
        tinymcp::McpAuthConfig::BearerToken { token: "t".into() }
    );
}

#[test]
fn every_credential_kind_converts() {
    // A variant dropped here silently sends no credential at all, which
    // surfaces as an unexplained 401.
    let cases = [
        (McpAuthConfig::None, tinymcp::McpAuthConfig::None),
        (
            McpAuthConfig::BearerToken { token: "t".into() },
            tinymcp::McpAuthConfig::BearerToken { token: "t".into() },
        ),
        (
            McpAuthConfig::Basic {
                username: "u".into(),
                password: "p".into(),
            },
            tinymcp::McpAuthConfig::Basic {
                username: "u".into(),
                password: "p".into(),
            },
        ),
        (
            McpAuthConfig::Header {
                name: "X-Key".into(),
                value: "v".into(),
            },
            tinymcp::McpAuthConfig::Header {
                name: "X-Key".into(),
                value: "v".into(),
            },
        ),
        (
            McpAuthConfig::QueryParam {
                name: "key".into(),
                value: "v".into(),
            },
            tinymcp::McpAuthConfig::QueryParam {
                name: "key".into(),
                value: "v".into(),
            },
        ),
    ];

    for (host, expected) in cases {
        let mut config = config_without_docs();
        let mut server = populated_server();
        server.auth = host;
        config.mcp_client.servers.push(server);

        assert_eq!(client_config(&config).servers[0].auth, expected);
    }
}

#[test]
fn a_multi_header_credential_keeps_every_header() {
    // A server wanting a client key and a client secret needs both; keeping
    // only the first is a 401 nobody can explain.
    let mut config = config_without_docs();
    let mut server = populated_server();
    server.auth = McpAuthConfig::Headers {
        headers: vec![
            HttpHeader {
                name: "X-Client-Key".into(),
                value: "k".into(),
            },
            HttpHeader {
                name: "Authorization".into(),
                value: "Bearer s".into(),
            },
        ],
    };
    config.mcp_client.servers.push(server);

    match &client_config(&config).servers[0].auth {
        tinymcp::McpAuthConfig::Headers { headers } => {
            assert_eq!(headers.len(), 2);
            assert!(headers
                .iter()
                .any(|header| { header.name == "X-Client-Key" && header.value == "k" }));
            assert!(headers
                .iter()
                .any(|header| { header.name == "Authorization" && header.value == "Bearer s" }));
        }
        other => panic!("expected several headers, got {other:?}"),
    }
}

#[test]
fn the_client_identity_survives_the_conversion() {
    // A remote server sees these and may log or display them.
    let mut config = config_without_docs();
    config.mcp_client.client_identity.name = "openhuman-core".into();
    config.mcp_client.client_identity.title = "OpenHuman Core MCP Client".into();
    config.mcp_client.client_identity.version = "9.9.9".into();

    let identity = client_config(&config).client_identity;

    assert_eq!(identity.name, "openhuman-core");
    assert_eq!(identity.title, "OpenHuman Core MCP Client");
    assert_eq!(identity.version, "9.9.9");
}

#[test]
fn the_registry_credentials_survive_the_conversion() {
    let mut config = config_without_docs();
    config.mcp_client.registry_auth.smithery_api_key = Some("smithery".into());
    config.mcp_client.registry_auth.mcp_official_base = Some("https://registry.test".into());
    config.mcp_client.registry_auth.mcp_official_token = Some("official".into());

    let auth = client_config(&config).registry_auth;

    assert_eq!(auth.smithery_api_key.as_deref(), Some("smithery"));
    assert_eq!(
        auth.mcp_official_base.as_deref(),
        Some("https://registry.test")
    );
    assert_eq!(auth.mcp_official_token.as_deref(), Some("official"));
}

#[test]
fn the_documentation_server_is_never_seeded_as_an_mcp_server() {
    // The docs are served by the hard-coded `gitbooks_*` tools; an MCP server
    // entry would make the first turn dial the endpoint for nothing.
    let mut config = Config::default();
    config.gitbooks.enabled = true;

    assert!(client_config(&config).servers.is_empty());
}

#[test]
fn a_disabled_mcp_section_carries_across() {
    let mut config = config_without_docs();
    config.mcp_client.enabled = false;

    assert!(!client_config(&config).enabled);
}

#[test]
fn a_host_opens_both_stores_under_the_workspace() {
    // Both live there, so the servers a user installed — and the record of what
    // they wrote — are found again after a restart.
    let temporary = tempfile::tempdir().expect("tempdir");
    let mut config = config_without_docs();
    config.workspace_dir = temporary.path().to_path_buf();

    let _host = McpHost::open(&config).expect("the host opens");

    assert!(tinymcp::Store::path_for(temporary.path()).exists());
    assert!(tinymcp::AuditStore::path_for(temporary.path()).exists());
}

#[test]
fn one_workspace_gets_one_service() {
    // Two callers naming the same workspace must meet: a connection opened
    // through one has to be visible through the other, and opening a service
    // runs migrations that no request path should pay for twice.
    let temporary = tempfile::tempdir().expect("tempdir");
    let mut config = config_without_docs();
    config.workspace_dir = temporary.path().to_path_buf();

    let first = super::for_config(&config).expect("the service opens");
    let second = super::for_config(&config).expect("the service opens again");

    assert!(std::sync::Arc::ptr_eq(&first, &second));
}

#[test]
fn two_workspaces_get_two_services() {
    // The reason the map is keyed at all. One process serves more than one
    // workspace over its life, and a caller naming the second must not be
    // handed the first's store.
    let first_dir = tempfile::tempdir().expect("tempdir");
    let second_dir = tempfile::tempdir().expect("tempdir");

    let mut config = config_without_docs();
    config.workspace_dir = first_dir.path().to_path_buf();
    let first = super::for_config(&config).expect("the first service opens");

    config.workspace_dir = second_dir.path().to_path_buf();
    let second = super::for_config(&config).expect("the second service opens");

    assert!(!std::sync::Arc::ptr_eq(&first, &second));
    assert!(tinymcp::Store::path_for(first_dir.path()).exists());
    assert!(tinymcp::Store::path_for(second_dir.path()).exists());
}

/// A map holding one service per named workspace.
fn hosts_for(workspaces: &[&std::path::Path]) -> HashMap<PathBuf, super::HostEntry> {
    workspaces
        .iter()
        .map(|workspace| {
            let mut config = config_without_docs();
            config.workspace_dir = workspace.to_path_buf();
            let client = super::client_config(&config);
            (
                workspace.to_path_buf(),
                super::HostEntry {
                    host: Arc::new(McpHost::open(&config).expect("the service opens")),
                    identity: client.client_identity,
                    proxy: client.proxy,
                },
            )
        })
        .collect()
}

#[test]
fn the_default_workspace_wins_when_one_is_set() {
    let first = tempfile::tempdir().expect("tempdir");
    let second = tempfile::tempdir().expect("tempdir");
    let hosts = hosts_for(&[first.path(), second.path()]);
    let expected = Arc::clone(&hosts[second.path()].host);

    let resolved = super::resolve(Some(second.path()), &hosts).expect("a service");

    assert!(Arc::ptr_eq(&resolved, &expected));
}

#[test]
fn the_only_open_service_answers_when_no_default_is_set() {
    // The agent's tool registry asks which MCP tools are live, and it asks by
    // server id alone. A process that opened a service without booting — a test,
    // or a host driving the library directly — would otherwise be told nothing
    // is connected while a connection sits in the map.
    let only = tempfile::tempdir().expect("tempdir");
    let hosts = hosts_for(&[only.path()]);
    let expected = Arc::clone(&hosts[only.path()].host);

    let resolved = super::resolve(None, &hosts).expect("the sole service");

    assert!(Arc::ptr_eq(&resolved, &expected));
}

#[test]
fn several_open_services_and_no_default_has_no_honest_answer() {
    // Picking one would be picking arbitrarily, and the caller cannot tell it
    // apart from the one it meant.
    let first = tempfile::tempdir().expect("tempdir");
    let second = tempfile::tempdir().expect("tempdir");

    assert!(super::resolve(None, &hosts_for(&[first.path(), second.path()])).is_none());
}

#[test]
fn nothing_open_answers_nothing() {
    assert!(super::resolve(None, &HashMap::new()).is_none());
}

#[test]
fn a_default_naming_a_workspace_nothing_opened_falls_through_to_the_rule() {
    // The default is set from a workspace that was opened, so this should not
    // happen — but if it ever did, answering the sole open service is better
    // than answering nothing.
    let only = tempfile::tempdir().expect("tempdir");
    let absent = tempfile::tempdir().expect("tempdir");
    let hosts = hosts_for(&[only.path()]);
    let expected = Arc::clone(&hosts[only.path()].host);

    let resolved = super::resolve(Some(absent.path()), &hosts).expect("the sole service");

    assert!(Arc::ptr_eq(&resolved, &expected));
}

#[test]
fn a_credentialed_plaintext_non_loopback_endpoint_is_refused() {
    use crate::config::McpAuthConfig as Auth;
    let mut config = config_without_docs();
    config.mcp_client.servers.push(HostServer {
        server: tinymcp_bus::McpServerConfig {
            name: "insecure".into(),
            endpoint: "http://example.test/mcp".into(),
            auth: Auth::BearerToken { token: "t".into() },
            ..Default::default()
        },
        ..Default::default()
    });

    let converted = client_config(&config);
    assert!(
        converted.servers.iter().all(|s| s.name != "insecure"),
        "credentialed plaintext-HTTP endpoint must not be converted"
    );
}

#[test]
fn a_credentialed_loopback_http_endpoint_is_allowed() {
    use crate::config::McpAuthConfig as Auth;
    let mut config = config_without_docs();
    config.mcp_client.servers.push(HostServer {
        server: tinymcp_bus::McpServerConfig {
            name: "local".into(),
            endpoint: "http://127.0.0.1:9000/mcp".into(),
            auth: Auth::Header {
                name: "X-Key".into(),
                value: "v".into(),
            },
            ..Default::default()
        },
        ..Default::default()
    });

    let converted = client_config(&config);
    assert!(
        converted.servers.iter().any(|s| s.name == "local"),
        "credentialed loopback HTTP endpoint must be preserved"
    );
}

#[test]
fn a_credentialed_https_endpoint_is_allowed() {
    use crate::config::McpAuthConfig as Auth;
    let mut config = config_without_docs();
    config.mcp_client.servers.push(HostServer {
        server: tinymcp_bus::McpServerConfig {
            name: "remote".into(),
            endpoint: "https://example.test/mcp".into(),
            auth: Auth::Basic {
                username: "u".into(),
                password: "p".into(),
            },
            ..Default::default()
        },
        ..Default::default()
    });

    let converted = client_config(&config);
    assert!(
        converted.servers.iter().any(|s| s.name == "remote"),
        "credentialed HTTPS endpoint must be preserved"
    );
}

/// A `config.toml` written before `McpServerConfig` became the contract's type
/// plus two host-only keys. It must parse to the same servers, and writing it
/// back must still read as the same document.
const EXISTING_CONFIG_TOML: &str = r#"
[mcp_client]
enabled = true

[mcp_client.client_identity]
name = "custom-name"

[mcp_client.registry_auth]
smithery_api_key = "smithery-key"

[[mcp_client.servers]]
name = "weather"
endpoint = "https://example.test/mcp"
description = "Weather lookups"
allowed_tools = ["forecast"]
disallowed_tools = ["debug"]
timeout_secs = 9
expose = "direct"
direct_tools = ["forecast"]

[mcp_client.servers.auth]
kind = "headers"
headers = [{ name = "X-Client-Key", value = "k" }, { name = "X-Org", value = "o" }]

[[mcp_client.servers]]
name = "local"
command = "npx"
args = ["-y", "weather-mcp"]
enabled = false

[mcp_client.servers.env]
API_KEY = "secret"
"#;

#[test]
fn an_existing_config_toml_parses_with_the_same_shape_and_defaults() {
    let config: Config = toml::from_str(EXISTING_CONFIG_TOML).expect("the existing shape parses");
    let servers = &config.mcp_client.servers;
    assert_eq!(servers.len(), 2);

    let weather = &servers[0];
    assert_eq!(weather.name, "weather");
    assert_eq!(weather.endpoint, "https://example.test/mcp");
    assert_eq!(weather.description.as_deref(), Some("Weather lookups"));
    assert_eq!(weather.allowed_tools, ["forecast"]);
    assert_eq!(weather.disallowed_tools, ["debug"]);
    assert_eq!(weather.timeout_secs, 9);
    assert!(weather.enabled, "enabled defaults to true");
    assert_eq!(weather.expose, crate::config::McpToolExposure::Direct);
    assert_eq!(weather.direct_tools, ["forecast"]);
    assert_eq!(
        weather.auth,
        McpAuthConfig::Headers {
            headers: vec![
                HttpHeader::new("X-Client-Key", "k"),
                HttpHeader::new("X-Org", "o"),
            ]
        }
    );

    let local = &servers[1];
    assert_eq!(local.command, "npx");
    assert_eq!(local.args, ["-y", "weather-mcp"]);
    assert!(!local.enabled);
    assert_eq!(local.timeout_secs, 30, "the timeout default is unchanged");
    assert_eq!(local.auth, McpAuthConfig::None);
    assert_eq!(local.expose, crate::config::McpToolExposure::Deferred);
    assert!(local.direct_tools.is_empty());
    assert_eq!(local.env.get("API_KEY").map(String::as_str), Some("secret"));

    // The identity block keeps this application's defaults for whatever the
    // file leaves out.
    assert_eq!(config.mcp_client.client_identity.name, "custom-name");
    assert_eq!(
        config.mcp_client.client_identity.title,
        "OpenHuman Core MCP Client"
    );
    assert_eq!(
        config.mcp_client.registry_auth.smithery_api_key.as_deref(),
        Some("smithery-key")
    );

    // And it converts to the module's configuration with the proxy defaulted.
    let mut config = config;
    config.gitbooks.enabled = false;
    let converted = client_config(&config);
    assert_eq!(converted.servers.len(), 2);
    assert_eq!(
        converted.registry_auth.smithery_api_key.as_deref(),
        Some("smithery-key")
    );
}

#[test]
fn a_config_with_servers_writes_back_the_same_keys() {
    let config: Config = toml::from_str(EXISTING_CONFIG_TOML).expect("parses");
    let written = toml::to_string(&config).expect("a flattened server serializes");
    let reread: Config = toml::from_str(&written).expect("the written file parses");
    let doc: toml::Table = toml::from_str(&written).expect("a table");

    let servers = doc["mcp_client"]["servers"].as_array().expect("servers");
    let first = servers[0].as_table().expect("a table");
    // Contract keys and host keys sit side by side in one table, as before.
    for key in [
        "name",
        "endpoint",
        "timeout_secs",
        "allowed_tools",
        "auth",
        "expose",
        "direct_tools",
    ] {
        assert!(first.contains_key(key), "missing `{key}` in {written}");
    }
    assert!(!first.contains_key("server"), "no wrapper key: {written}");
    assert_eq!(first["expose"].as_str(), Some("direct"));
    assert_eq!(first["auth"]["kind"].as_str(), Some("headers"));
    assert_eq!(reread.mcp_client.servers[0].name, "weather");
    assert_eq!(reread.mcp_client.servers[1].env["API_KEY"], "secret");
}
