//! Agent tools backed by the TinySearch module.
//!
//! Declarations come from `modules::search::configured_tool_specs`, which is
//! computed synchronously from config so a turn's tool list is stable and
//! needs no module round-trip. Each call re-reads the live config and goes
//! through `modules::search::execute_tool`, so a provider or login change is
//! honoured on the next call without rebuilding the session.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tinysearch_bus::{errors, ExecuteToolRequest, ToolSpec};
use tinytools::{Tool, ToolCallOptions, ToolCategory, ToolExposure, ToolResult};

use crate::config::Config;

/// One TinySearch tool (a role tool such as `web_search_tool`, or a provider
/// tool in `all_tools` presentation).
/// What a call reports once every provider has already answered
/// "unavailable" in this session. It tells the model to stop rather than to
/// wait, because nothing about the session will change the answer (#6991).
pub const SEARCH_EXHAUSTED_MESSAGE: &str =
    "Web search is not available in this session. Do not call it again \u{2014} \
     answer from the material you already have.";

pub struct TinySearchTool {
    spec: ToolSpec,
    /// Spawn-time config. `None` for a deferred instance rebuilt from a
    /// recorded transcript, which resolves the live config per call.
    config: Option<Arc<Config>>,
    exposure: ToolExposure,
    /// Latched when a call reports that no provider could answer. Shared by
    /// the search tools built for one session instance, so the model is told
    /// once and the later calls cost no module round-trip.
    ///
    /// A credential alone makes the managed route look reachable
    /// (`providers::backend_credential_available`), so a deployment that is
    /// offline, firewalled, out of balance or holding a dead key offers these
    /// tools on every turn and fails every call. The agent then spends turns
    /// on a tool that cannot work, at the moment it is least sure what to do.
    exhausted: Arc<AtomicBool>,
}

impl TinySearchTool {
    pub fn new(config: Arc<Config>, spec: ToolSpec) -> Self {
        Self {
            spec,
            config: Some(config),
            exposure: ToolExposure::Direct,
            exhausted: Arc::new(AtomicBool::new(false)),
        }
    }

    /// A tool recorded in a resumed thread's transcript. It keeps the recorded
    /// declaration and resolves the live config on every call.
    pub fn recorded(spec: ToolSpec) -> Self {
        Self {
            spec,
            config: None,
            exposure: ToolExposure::Direct,
            exhausted: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Recorded tools that share one exhaustion latch, the way the tools of a
    /// live session do: the roles are three declarations over the same
    /// providers, so one of them finding nothing settles it for all three.
    pub fn recorded_batch(specs: impl IntoIterator<Item = ToolSpec>) -> Vec<Self> {
        let exhausted = Arc::new(AtomicBool::new(false));
        specs
            .into_iter()
            .map(|spec| Self {
                spec,
                config: None,
                exposure: ToolExposure::Direct,
                exhausted: exhausted.clone(),
            })
            .collect()
    }

    /// Whether this session already learned that no provider can answer.
    pub(crate) fn is_exhausted(&self) -> bool {
        self.exhausted.load(Ordering::Relaxed)
    }

    /// Record that no provider could answer, for the tools sharing this latch.
    pub(crate) fn mark_exhausted(&self) {
        self.exhausted.store(true, Ordering::Relaxed);
    }

    pub fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    async fn live_config(&self) -> anyhow::Result<Config> {
        match &self.config {
            Some(config) => Ok(config.as_ref().clone()),
            None => crate::config::rpc::load_config_with_timeout()
                .await
                .map_err(|error| anyhow::anyhow!(error)),
        }
    }
}

/// Map a module error to a message the model can act on. The module prefixes
/// classified failures with `tinysearch.<code>:`; the detail after the prefix
/// may echo the query, so it is not logged.
pub fn user_facing_error(error: &str) -> String {
    match error_code(error) {
        Some(code) if code == errors::INSUFFICIENT_BALANCE => {
            "Web search is unavailable: the TinyHumans balance is too low for managed search. \
             Top up the balance, or add your own provider key under Connections → Search."
                .to_string()
        }
        Some(code) if code == errors::RATE_LIMITED => {
            "Web search is rate limited right now. Wait a moment and try again.".to_string()
        }
        Some(code) if code == errors::UNAVAILABLE => SEARCH_EXHAUSTED_MESSAGE.to_string(),
        Some(code) if code == errors::INVALID_ARGUMENTS => {
            let marker = format!("{}{code}: ", errors::PREFIX);
            let detail = error
                .split_once(marker.as_str())
                .map(|(_, detail)| detail)
                .unwrap_or(error);
            format!("The search request was rejected: {detail}")
        }
        _ => format!("Web search failed: {error}"),
    }
}

/// The classified error code in a module error message, if any.
pub fn error_code(error: &str) -> Option<&'static str> {
    errors::code_of(error)
}

/// Whether this failure means no provider can answer for the rest of the
/// session, rather than something a later call could get past.
///
/// Only the "every provider is unavailable" verdict qualifies. A rate limit
/// clears on its own, and a low balance or a rejected argument has its own
/// message telling the caller what to change, so neither latches.
pub(crate) fn exhausts_providers(error: &str) -> bool {
    error_code(error) == Some(errors::UNAVAILABLE)
}

#[async_trait]
impl Tool for TinySearchTool {
    fn name(&self) -> &str {
        &self.spec.name
    }

    fn description(&self) -> &str {
        &self.spec.description
    }

    fn parameters_schema(&self) -> Value {
        self.spec.parameters.clone()
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Workflow
    }

    fn exposure(&self) -> ToolExposure {
        self.exposure
    }

    fn supports_markdown(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _args: &Value) -> bool {
        true
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        self.execute_with_options(args, ToolCallOptions::default())
            .await
    }

    async fn execute_with_options(
        &self,
        args: Value,
        options: ToolCallOptions,
    ) -> anyhow::Result<ToolResult> {
        if self.is_exhausted() {
            tracing::debug!(
                tool = %self.spec.name,
                "[search][tool] refused: no provider answered earlier in this session"
            );
            return Ok(ToolResult::failed(SEARCH_EXHAUSTED_MESSAGE.to_string()));
        }
        let config = self.live_config().await?;
        let subject = super::render::subject(&args);
        let max_results = args
            .get("max_results")
            .and_then(Value::as_u64)
            .map(|n| n as usize)
            .unwrap_or(config.search.max_results)
            .clamp(1, 20);
        tracing::debug!(
            tool = %self.spec.name,
            subject_len = subject.chars().count(),
            "[search][tool] execute"
        );
        let request = ExecuteToolRequest {
            name: self.spec.name.clone(),
            arguments: args,
        };
        if let Some(message) = local_only_search_block(&request.name) {
            return Ok(ToolResult::error(message));
        }
        match crate::modules::search::execute_tool(&config, request).await {
            Ok(response) => {
                tracing::debug!(
                    tool = %self.spec.name,
                    provider = %response.provider,
                    fallbacks = response.fallback_from.len(),
                    results = response.results.len(),
                    "[search][tool] completed"
                );
                Ok(super::render::render(
                    &response,
                    &subject,
                    max_results,
                    options.prefer_markdown,
                ))
            }
            Err(error) => {
                if exhausts_providers(&error) {
                    self.mark_exhausted();
                }
                tracing::warn!(
                    tool = %self.spec.name,
                    code = error_code(&error).unwrap_or("unclassified"),
                    exhausted = self.is_exhausted(),
                    "[search][tool] failed"
                );
                Ok(if exhausts_providers(&error) {
                    ToolResult::failed(user_facing_error(&error))
                } else {
                    ToolResult::error(user_facing_error(&error))
                })
            }
        }
    }
}

pub(crate) fn local_only_search_block(tool_name: &str) -> Option<String> {
    crate::security::egress::local_only_tool_block(&crate::security::egress::EgressDescriptor::new(
        "tinysearch",
        tool_name,
        true,
        crate::security::egress::EgressReason::ToolCall,
        vec![crate::security::egress::DataKind::ToolArguments],
    ))
}

/// Build the agent's search tools from config. Empty when search is off or no
/// provider is usable.
pub fn build_search_tools(config: &Config) -> Vec<Box<dyn Tool>> {
    let specs = crate::modules::search::configured_tool_specs(config);
    tracing::debug!(
        tools = ?specs.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        "[search][tool] registered search tools"
    );
    let shared = Arc::new(config.clone());
    let exhausted = Arc::new(AtomicBool::new(false));
    specs
        .into_iter()
        .map(|spec| {
            Box::new(TinySearchTool {
                spec,
                config: Some(shared.clone()),
                exposure: ToolExposure::Direct,
                exhausted: exhausted.clone(),
            }) as Box<dyn Tool>
        })
        .collect()
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
