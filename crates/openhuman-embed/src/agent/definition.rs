//! What an agent *is*: prompt, tool scope, sandbox mode, iteration cap.
//!
//! A thin re-spelling of the core's
//! [`AgentDefinition`](openhuman_core::agent::harness::definition::AgentDefinition)
//! so the library surface does not track that thirty-field struct
//! field-by-field. The default is the built-in orchestrator's definition
//! under the agent's own id — the same dynamic prompt and delegation
//! surface the desktop's main agent runs with — except that **every
//! registered tool is visible** ([`ToolScopeSpec::Wildcard`]). The desktop
//! orchestrator names its direct tools and delegates the rest (MCP bridge,
//! integrations) to specialists; a library agent that declared an MCP
//! server expects to call it, so the host narrows with
//! [`AgentDefinitionSpec::tools`] rather than widening.

use openhuman_core::agent::harness::definition::{
    AgentDefinition, AgentDefinitionRegistry, PromptSource, SandboxMode, ToolScope,
};

use super::AgentError;

/// Which tools an agent may call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolScopeSpec {
    /// Every tool the runtime registered (subject to `disallow_tools`).
    Wildcard,
    /// Exactly these tool names; unknown names are dropped at build time.
    Named(Vec<String>),
    /// The host's tools and nothing else.
    ///
    /// For an agent that must never act — a reviewer reading an untrusted
    /// diff. The registry the model sees is built from the tools the host
    /// supplies ([`AgentSpec::tools`](super::AgentSpec::tools),
    /// [`Agent::attach_tools`](super::Agent::attach_tools)) alone: no
    /// config-derived, delegation, memory, skill or MCP tool, and a
    /// deny-by-default gate refuses any other name the model calls. The agent
    /// also runs read-only (access tier and sandbox) whatever
    /// [`Access`](crate::Access) it was given. Host tools should themselves be
    /// read-only: host-only bounds *which* tools exist, not what they do.
    HostOnly,
}

/// How an agent's shell and file tools are confined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SandboxModeSpec {
    /// Only the access tier and path policy apply.
    #[default]
    None,
    /// Write and execute tools are filtered out.
    ReadOnly,
    /// Commands run under the platform jail or Docker backend.
    Sandboxed,
}

/// Builder for an agent's definition. See the module docs for the default.
#[derive(Debug, Clone, Default)]
pub struct AgentDefinitionSpec {
    system_prompt: Option<String>,
    /// The prompt is the whole system prompt, with nothing composed around it.
    bare_prompt: bool,
    tools: Option<ToolScopeSpec>,
    disallowed_tools: Vec<String>,
    tool_rules: Option<tinytools::ToolRules>,
    sandbox: SandboxModeSpec,
    max_iterations: Option<usize>,
    temperature: Option<f64>,
    display_name: Option<String>,
    when_to_use: Option<String>,
}

impl AgentDefinitionSpec {
    /// The orchestrator's definition, to be narrowed.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the dynamic system prompt with this fixed text.
    ///
    /// The identity, memory and safety sections the orchestrator prompt
    /// carries are kept around it; only the body changes.
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self.bare_prompt = false;
        self
    }

    /// Make `prompt` the whole system prompt, verbatim.
    ///
    /// Unlike [`system_prompt`](Self::system_prompt), no orchestrator
    /// identity, safety, tools, workspace or memory section is composed
    /// around it, and no memory context is injected into a new session. The
    /// host owns every byte the model is told. Tool schemas still travel in
    /// the request's `tools` field.
    pub fn bare_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self.bare_prompt = true;
        self
    }

    /// Whether this is a [`ToolScopeSpec::HostOnly`] definition.
    pub(crate) fn is_host_only(&self) -> bool {
        matches!(self.tools, Some(ToolScopeSpec::HostOnly))
    }

    /// Restrict which tools the agent may call.
    pub fn tools(mut self, scope: ToolScopeSpec) -> Self {
        self.tools = Some(scope);
        self
    }

    /// Hide these tools even when the scope would include them.
    pub fn disallow_tools<I, S>(mut self, tools: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.disallowed_tools
            .extend(tools.into_iter().map(Into::into));
        self
    }

    /// Pattern rules narrowing which tools the agent may see and call — on
    /// the catalogue, `tool_search` and every call. Stacks with
    /// [`Self::tools`], [`Self::disallow_tools`] and the runtime config's
    /// `[tool_rules]`; it can only narrow. See [`tinytools::ToolRules`].
    pub fn tool_rules(mut self, rules: tinytools::ToolRules) -> Self {
        self.tool_rules = Some(rules);
        self
    }

    /// Confine the agent's shell and file tools.
    pub fn sandbox(mut self, mode: SandboxModeSpec) -> Self {
        self.sandbox = mode;
        self
    }

    /// Cap the tool-call iterations per turn.
    pub fn max_iterations(mut self, n: usize) -> Self {
        self.max_iterations = Some(n);
        self
    }

    /// Default sampling temperature.
    pub fn temperature(mut self, t: f64) -> Self {
        self.temperature = Some(t);
        self
    }

    /// Human-readable name shown in transcripts and delegation catalogs.
    pub fn display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = Some(name.into());
        self
    }

    /// One-line description of when this agent should be used.
    pub fn when_to_use(mut self, text: impl Into<String>) -> Self {
        self.when_to_use = Some(text.into());
        self
    }

    /// Materialize the core definition for agent `id`.
    pub(crate) fn into_core(self, id: &str) -> Result<AgentDefinition, AgentError> {
        let host_only = self.is_host_only();
        let registry = AgentDefinitionRegistry::builtins_only();
        let mut def = registry.get(ORCHESTRATOR_ID).cloned().ok_or_else(|| {
            AgentError::Invalid("built-in orchestrator definition is missing".to_string())
        })?;
        def.id = id.to_string();
        def.display_name = Some(self.display_name.unwrap_or_else(|| id.to_string()));
        if let Some(text) = self.when_to_use {
            def.when_to_use = text;
        }
        match self.system_prompt {
            Some(prompt) if self.bare_prompt => {
                def.system_prompt = PromptSource::Verbatim(prompt);
                def.omit_identity = true;
                def.omit_safety_preamble = true;
                def.omit_memory_context = true;
            }
            Some(prompt) => def.system_prompt = PromptSource::Inline(prompt),
            // The orchestrator's dynamic prompt describes a delegation and
            // memory surface a host-only agent does not have.
            None if host_only => {
                return Err(AgentError::Invalid(
                    "a HostOnly agent needs its own prompt: set bare_prompt or system_prompt"
                        .to_string(),
                ));
            }
            None => {}
        }
        def.tools = match self.tools.unwrap_or(ToolScopeSpec::Wildcard) {
            ToolScopeSpec::Wildcard => ToolScope::Wildcard,
            ToolScopeSpec::Named(names) => ToolScope::Named(names),
            // Zero tools; the host's names join at session build.
            ToolScopeSpec::HostOnly => ToolScope::Named(Vec::new()),
        };
        def.disallowed_tools.extend(self.disallowed_tools);
        if let Some(rules) = self.tool_rules {
            def.tool_rules = Some(rules);
        }
        def.sandbox_mode = match self.sandbox {
            _ if host_only => SandboxMode::ReadOnly,
            SandboxModeSpec::None => SandboxMode::None,
            SandboxModeSpec::ReadOnly => SandboxMode::ReadOnly,
            SandboxModeSpec::Sandboxed => SandboxMode::Sandboxed,
        };
        if host_only {
            def.subagents.clear();
            def.extra_tools.clear();
            def.deferred_tools.clear();
        }
        if let Some(n) = self.max_iterations {
            def.max_iterations = n;
        }
        if let Some(t) = self.temperature {
            def.temperature = t;
        }
        log::debug!(
            "[embed][agent] definition id={id} prompt={} tools={:?} sandbox={:?} max_iterations={}",
            match &def.system_prompt {
                PromptSource::Inline(_) => "inline",
                PromptSource::File { .. } => "file",
                PromptSource::Dynamic(_) => "dynamic",
                PromptSource::Verbatim(_) => "verbatim",
            },
            def.tools,
            def.sandbox_mode,
            def.max_iterations
        );
        Ok(def)
    }
}

/// The built-in definition every embedded agent starts from.
const ORCHESTRATOR_ID: &str = "orchestrator";

#[cfg(test)]
#[path = "definition_tests.rs"]
mod tests;
