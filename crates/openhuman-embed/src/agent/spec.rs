//! Everything a host says about one agent before it exists.
//!
//! An [`AgentSpec`] is pure data: it is applied by
//! [`Runtime::agent`](crate::Runtime::agent), in a fixed order, onto a clone
//! of the runtime's base config — access tier, provider model, MCP servers,
//! then the [`config`](AgentSpec::config) escape hatch last — and onto a
//! [`AgentDefinitionSpec`].

use std::path::PathBuf;

use openhuman_core::config::Config;
use openhuman_core::core::runtime::DomainSet;
use openhuman_core::security::TrustedAccess;
use openhuman_core::tools::toolpacks::ToolGroups;

use super::AgentDefinitionSpec;
use crate::harness::{Access, Provider};

/// The [`AgentSpec::config`] escape hatch, applied last onto the agent's config.
type ConfigEdit = Box<dyn FnOnce(&mut Config) + Send>;

/// Where [`AgentSpec::skills_dir`] bundles are copied.
#[cfg(feature = "skills")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SkillsDest {
    /// `<workspace>/agents/<id>/skills/` — seen only by this agent.
    AgentLocal,
    /// `<workspace>/skills/` — the legacy workspace root the one-agent
    /// [`Harness`](crate::Harness) always used; kept for its callers.
    WorkspaceLegacy,
}

/// Description of an agent to instantiate on a [`Runtime`](crate::Runtime).
pub struct AgentSpec {
    id: String,
    definition: AgentDefinitionSpec,
    provider: Option<Provider>,
    access: Option<Access>,
    tool_groups: Option<ToolGroups>,
    domains: Option<DomainSet>,
    #[cfg(feature = "mcp")]
    mcp_servers: Vec<crate::harness::McpServer>,
    #[cfg(feature = "skills")]
    skills_dir: Option<PathBuf>,
    #[cfg(feature = "skills")]
    skills_dest: SkillsDest,
    include_user_skills: bool,
    action_dir: Option<PathBuf>,
    trusted: Vec<(String, TrustedAccess)>,
    config_fn: Option<ConfigEdit>,
}

impl AgentSpec {
    /// An agent named `id`, with every setting at the runtime's default.
    ///
    /// The id must match `^[a-z0-9][a-z0-9_-]{0,63}$` (checked at
    /// [`Runtime::agent`](crate::Runtime::agent)); it names the agent's
    /// directories and transcripts. Avoid the built-in ids (`orchestrator`,
    /// `summarizer`, …): the runtime-wide delegation catalog resolves those
    /// to the shipped definitions.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            definition: AgentDefinitionSpec::new(),
            provider: None,
            access: None,
            tool_groups: None,
            domains: None,
            #[cfg(feature = "mcp")]
            mcp_servers: Vec::new(),
            #[cfg(feature = "skills")]
            skills_dir: None,
            #[cfg(feature = "skills")]
            skills_dest: SkillsDest::AgentLocal,
            include_user_skills: false,
            action_dir: None,
            trusted: Vec::new(),
            config_fn: None,
        }
    }

    /// The id given to [`new`](Self::new).
    pub fn id(&self) -> &str {
        &self.id
    }

    /// What the agent is: prompt, tool scope, sandbox, iteration cap.
    pub fn definition(mut self, definition: AgentDefinitionSpec) -> Self {
        self.definition = definition;
        self
    }

    /// Shorthand for [`AgentDefinitionSpec::system_prompt`].
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.definition = self.definition.system_prompt(prompt);
        self
    }

    /// Which model answers and where the request goes. Unset means the
    /// runtime's default provider — the managed TinyHumans backend via the
    /// runtime's API key when no route was given.
    pub fn provider(mut self, provider: Provider) -> Self {
        self.provider = Some(provider);
        self
    }

    /// Pin the model without changing the route.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        let provider = self.provider.take().unwrap_or_else(Provider::inherit);
        self.provider = Some(provider.model(model));
        self
    }

    /// What the agent is allowed to do. Unset means the runtime's default.
    pub fn access(mut self, access: Access) -> Self {
        self.access = Some(access);
        self
    }

    /// Narrow how tool groups are disclosed to this agent. Must not advertise
    /// a group the runtime withheld or turned off.
    pub fn tool_groups(mut self, groups: ToolGroups) -> Self {
        self.tool_groups = Some(groups);
        self
    }

    /// Narrow which domain families this agent sees. Must be a subset of the
    /// runtime's.
    pub fn domains(mut self, domains: DomainSet) -> Self {
        self.domains = Some(domains);
        self
    }

    /// Declare an MCP server this agent may call tools on. Call repeatedly to
    /// add several. Other agents on the runtime do not see it.
    #[cfg(feature = "mcp")]
    pub fn mcp(mut self, server: crate::harness::McpServer) -> Self {
        self.mcp_servers.push(server);
        self
    }

    /// Make the skill bundles in `dir` available to this agent alone.
    ///
    /// Copied into `<workspace>/agents/<id>/skills/` — copied rather
    /// than linked because skill discovery rejects symlinked bundles.
    #[cfg(feature = "skills")]
    pub fn skills_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.skills_dir = Some(dir.into());
        self.skills_dest = SkillsDest::AgentLocal;
        self
    }

    /// [`skills_dir`](Self::skills_dir), but into the workspace's shared
    /// `skills/` root. What the one-agent [`Harness`](crate::Harness) does.
    #[cfg(feature = "skills")]
    pub(crate) fn skills_dir_legacy(mut self, dir: PathBuf) -> Self {
        self.skills_dir = Some(dir);
        self.skills_dest = SkillsDest::WorkspaceLegacy;
        self
    }

    /// Also let this agent discover the operator's user-scope skills
    /// (`~/.openhuman/skills`, `~/.agents/skills`). Off by default: an
    /// embedded agent sees what its host installed, not what the machine's
    /// user did.
    pub fn include_user_skills(mut self, include: bool) -> Self {
        self.include_user_skills = include;
        self
    }

    /// The agent's read/write root for acting tools.
    ///
    /// Defaults to `<root>/agents/<id>/action` (or, on an inherited
    /// workspace, `<action_dir>/agents/<id>`). Point it at the project the
    /// agent should work in — this is the directory whose contents it can
    /// change.
    pub fn action_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.action_dir = Some(dir.into());
        self
    }

    /// Grant access to a directory outside the action root. Convenience over
    /// [`Access::trust`].
    pub fn trust(mut self, path: impl Into<String>, access: TrustedAccess) -> Self {
        self.trusted.push((path.into(), access));
        self
    }

    /// Arbitrary edits to the agent's config, applied last.
    ///
    /// The escape hatch for the config fields the spec does not model — not
    /// a way to bypass them. `config_path` is reset afterwards: credentials
    /// and the keyring resolve against it and every agent shares them.
    pub fn config(mut self, f: impl FnOnce(&mut Config) + Send + 'static) -> Self {
        self.config_fn = Some(Box::new(f));
        self
    }

    // ── accessors for the build step ─────────────────────────────────────

    pub(crate) fn into_parts(self) -> AgentSpecParts {
        AgentSpecParts {
            id: self.id,
            definition: self.definition,
            provider: self.provider,
            access: self.access,
            tool_groups: self.tool_groups,
            domains: self.domains,
            #[cfg(feature = "mcp")]
            mcp_servers: self.mcp_servers,
            #[cfg(feature = "skills")]
            skills_dir: self.skills_dir,
            #[cfg(feature = "skills")]
            skills_dest: self.skills_dest,
            include_user_skills: self.include_user_skills,
            action_dir: self.action_dir,
            trusted: self.trusted,
            config_fn: self.config_fn,
        }
    }
}

/// The spec's fields, destructured for [`super::build::instantiate`].
pub(crate) struct AgentSpecParts {
    pub(crate) id: String,
    pub(crate) definition: AgentDefinitionSpec,
    pub(crate) provider: Option<Provider>,
    pub(crate) access: Option<Access>,
    pub(crate) tool_groups: Option<ToolGroups>,
    pub(crate) domains: Option<DomainSet>,
    #[cfg(feature = "mcp")]
    pub(crate) mcp_servers: Vec<crate::harness::McpServer>,
    #[cfg(feature = "skills")]
    pub(crate) skills_dir: Option<PathBuf>,
    #[cfg(feature = "skills")]
    pub(crate) skills_dest: SkillsDest,
    pub(crate) include_user_skills: bool,
    pub(crate) action_dir: Option<PathBuf>,
    pub(crate) trusted: Vec<(String, TrustedAccess)>,
    pub(crate) config_fn: Option<ConfigEdit>,
}

impl std::fmt::Debug for AgentSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The provider may carry a bearer; the config closure is opaque.
        f.debug_struct("AgentSpec")
            .field("id", &self.id)
            .field("access", &self.access)
            .field("action_dir", &self.action_dir)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "spec_tests.rs"]
mod tests;
