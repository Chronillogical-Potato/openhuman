//! MCP client, server, auth, and GitBooks config types.

use super::super::defaults;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct GitbooksConfig {
    /// When `true`, register `gitbooks_search` and `gitbooks_get_page`.
    #[serde(default = "defaults::default_true")]
    pub enabled: bool,
    /// MCP endpoint URL for the OpenHuman GitBook docs.
    #[serde(default = "default_gitbooks_endpoint")]
    pub endpoint: String,
    /// Per-request timeout in seconds.
    #[serde(default = "default_gitbooks_timeout_secs")]
    pub timeout_secs: u64,
}

fn default_gitbooks_endpoint() -> String {
    "https://tinyhumans.gitbook.io/openhuman/~gitbook/mcp".into()
}

fn default_gitbooks_timeout_secs() -> u64 {
    30
}

impl Default for GitbooksConfig {
    fn default() -> Self {
        Self {
            enabled: defaults::default_true(),
            endpoint: default_gitbooks_endpoint(),
            timeout_secs: default_gitbooks_timeout_secs(),
        }
    }
}

// The wire contract's shapes, shared with `tinymcp`: what a server is called,
// how to reach it, and how to authenticate. The TOML spelling is the
// contract's, so a config file and the module cannot drift apart.
pub use tinymcp_bus::{HttpHeader, McpAuthConfig, McpRegistryAuthConfig};

/// One configured MCP server: the contract's [`tinymcp_bus::McpServerConfig`]
/// (flattened, so the TOML keys are unchanged) plus how this application
/// exposes its tools to the model.
///
/// Dereferences to the contract type, so `server.name`, `server.endpoint` and
/// the rest read as they always have.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct McpServerConfig {
    /// Name, endpoint or command, environment, allow/deny lists, timeout and
    /// auth: everything the MCP client needs to reach the server.
    #[serde(flatten)]
    pub server: tinymcp_bus::McpServerConfig,
    /// How this server's tools reach the model. Every remote tool becomes its
    /// own `mcp_<server>_<tool>` tool; `deferred` (the default) leaves them
    /// out of the catalogue until `tool_search` finds them, `direct` sends
    /// them every turn.
    #[serde(default)]
    pub expose: McpToolExposure,
    /// Remote tool names sent to the model every turn even when `expose` is
    /// `deferred` — the handful this server is used for most.
    #[serde(default)]
    pub direct_tools: Vec<String>,
}

impl std::ops::Deref for McpServerConfig {
    type Target = tinymcp_bus::McpServerConfig;

    fn deref(&self) -> &Self::Target {
        &self.server
    }
}

impl std::ops::DerefMut for McpServerConfig {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.server
    }
}

impl From<tinymcp_bus::McpServerConfig> for McpServerConfig {
    fn from(server: tinymcp_bus::McpServerConfig) -> Self {
        Self {
            server,
            ..Self::default()
        }
    }
}

/// How a configured MCP server's tools enter the model's catalogue.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum McpToolExposure {
    /// Found through `tool_search`, then callable by name.
    #[default]
    Deferred,
    /// Sent to the model every turn.
    Direct,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct McpClientIdentityConfig {
    /// Client name sent during `initialize.clientInfo.name`.
    #[serde(default = "default_mcp_client_name")]
    pub name: String,
    /// Client title sent during `initialize.clientInfo.title`.
    #[serde(default = "default_mcp_client_title")]
    pub title: String,
    /// Client version sent during `initialize.clientInfo.version`.
    #[serde(default = "default_mcp_client_version")]
    pub version: String,
}

fn default_mcp_client_name() -> String {
    "openhuman-core".into()
}

fn default_mcp_client_title() -> String {
    "OpenHuman Core MCP Client".into()
}

fn default_mcp_client_version() -> String {
    env!("CARGO_PKG_VERSION").into()
}

impl Default for McpClientIdentityConfig {
    fn default() -> Self {
        Self {
            name: default_mcp_client_name(),
            title: default_mcp_client_title(),
            version: default_mcp_client_version(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct McpClientConfig {
    /// When `true`, register the generic MCP bridge tools and expose
    /// configured remote MCP servers to the agent runtime.
    #[serde(default = "defaults::default_true")]
    pub enabled: bool,
    /// Named remote MCP servers accessible via `mcp_list_*` /
    /// `mcp_call_tool`.
    #[serde(default)]
    pub servers: Vec<McpServerConfig>,
    /// Identity block sent during initialize.
    #[serde(default)]
    pub client_identity: McpClientIdentityConfig,
    /// Optional auth/overrides for the MCP *registry* browse APIs (Smithery +
    /// the official modelcontextprotocol/registry). Each value falls back to
    /// the corresponding env var when unset (issue #3039 gap A6).
    #[serde(default)]
    pub registry_auth: McpRegistryAuthConfig,
}

impl Default for McpClientConfig {
    fn default() -> Self {
        Self {
            enabled: defaults::default_true(),
            servers: Vec::new(),
            client_identity: McpClientIdentityConfig::default(),
            registry_auth: McpRegistryAuthConfig::default(),
        }
    }
}
