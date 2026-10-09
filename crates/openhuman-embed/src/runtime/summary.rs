//! [`BuilderSummary`]: a read-only view of what a [`RuntimeBuilder`] carries.
//!
//! For the layers above embed (`openhuman-tinyhumans`, `openhuman-rpc`) to
//! assert, in their own tests, that they configured a builder the way they
//! claim — the builder's fields are private to this crate. It shows which
//! options are set, never their secrets: no API key, config or session.

use std::path::PathBuf;

use openhuman_core::core::all::DomainGroup;
use openhuman_core::core::runtime::{DomainSet, ServiceSet, TokenSource};
use openhuman_core::core::types::HostKind;

use super::builder::ConfigSource;
use super::RuntimeBuilder;
use crate::harness::Workspace;

/// What a [`RuntimeBuilder`] is set to; see the module docs.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct BuilderSummary {
    pub host_kind: HostKind,
    pub workspace: Workspace,
    pub workspace_dir: Option<PathBuf>,
    pub action_dir: Option<PathBuf>,
    pub config_source: ConfigSource,
    pub domains: Option<DomainSet>,
    pub services: Option<ServiceSet>,
    /// `true` for [`TokenSource::Fixed`].
    pub fixed_token: bool,
    pub listen_host: Option<String>,
    pub listen_port: Option<u16>,
    pub has_api_key: bool,
    pub has_backend_transport: bool,
    pub has_memory_engine: bool,
    pub has_session_store: bool,
    /// The group of each controller extension, in registration order.
    pub controller_extensions: Vec<DomainGroup>,
    /// The [`kind`](tinytools::ToolRanker::kind) of the tool ranker.
    pub tool_ranker: Option<String>,
    pub post_turn_hooks: Vec<String>,
    pub tool_hooks: Vec<String>,
    pub has_server_launcher: bool,
    pub has_live_policy: bool,
}

impl RuntimeBuilder {
    /// A read-only view of this builder's settings, for the layered crates'
    /// tests. Not part of the embedding API.
    #[doc(hidden)]
    pub fn summary(&self) -> BuilderSummary {
        BuilderSummary {
            host_kind: self.host_kind,
            workspace: self.workspace.clone(),
            workspace_dir: self.workspace_dir.clone(),
            action_dir: self.action_dir.clone(),
            config_source: self.config_source,
            domains: self.domains,
            services: self.services,
            fixed_token: matches!(self.token, TokenSource::Fixed(_)),
            listen_host: self.listen_host.clone(),
            listen_port: self.listen_port,
            has_api_key: self.api_key.is_some(),
            has_backend_transport: self.backend_transport.is_some(),
            has_memory_engine: self.memory_engine.is_some(),
            has_session_store: self.session_store.is_some(),
            controller_extensions: self
                .seams
                .controller_extensions
                .iter()
                .map(|ext| ext.group)
                .collect(),
            tool_ranker: self
                .seams
                .tool_ranker
                .as_ref()
                .map(|ranker| ranker.kind().to_string()),
            post_turn_hooks: self
                .seams
                .post_turn_hooks
                .iter()
                .map(|hook| hook.name().to_string())
                .collect(),
            tool_hooks: self
                .seams
                .tool_hooks
                .iter()
                .map(|hook| hook.name().to_string())
                .collect(),
            has_server_launcher: self.seams.server_launcher.is_some(),
            has_live_policy: self.seams.live_policy.is_some(),
        }
    }
}

#[cfg(test)]
#[path = "summary_tests.rs"]
mod tests;
