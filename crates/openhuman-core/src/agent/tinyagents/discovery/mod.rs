//! What ranks a `tool_search` in this process.
//!
//! The tinyagents harness owns tool discovery — the intrinsic `tool_search` /
//! `tool_call` bridge over every `ToolExposure::Deferred` registration, a BM25
//! catalogue, and a slot for a host ranker (`tool::discover`). This module is
//! the host's side of that slot: which [`ToolRanker`] the process installed
//! (a decision model such as Jev, installed by `openhuman-tinyhumans`; the
//! core itself installs none) and how `agent.tool_search` in the config says
//! to use it. [`discovery_policy`] turns the two into the
//! [`ToolDiscoveryPolicy`] every turn harness runs with.
//!
//! Process-wide, like the backend transport, because the credential a
//! decision-model ranker needs is a property of the process's signed-in
//! user, and the harness is assembled per turn without a config in hand.

use std::sync::{Arc, OnceLock, RwLock};

use tinyagents_harness::tool::discover::{DiscoveryRankMode, ToolDiscoveryPolicy};
use tinytools::ToolRanker;

use crate::config::schema::ToolSearchConfig;

static RANKER: OnceLock<RwLock<Option<Arc<dyn ToolRanker>>>> = OnceLock::new();
static SETTINGS: OnceLock<RwLock<ToolSearchConfig>> = OnceLock::new();

fn ranker_slot() -> &'static RwLock<Option<Arc<dyn ToolRanker>>> {
    RANKER.get_or_init(|| RwLock::new(None))
}

fn settings_slot() -> &'static RwLock<ToolSearchConfig> {
    SETTINGS.get_or_init(|| RwLock::new(ToolSearchConfig::default()))
}

/// Install `ranker` as the process-wide `tool_search` ranker, replacing any
/// previous one. A ranker that fails at search time falls back to BM25 in
/// the harness, so installing one never makes a search fail.
pub fn install_tool_ranker(ranker: Arc<dyn ToolRanker>) {
    let kind = ranker.kind();
    let previous = ranker_slot()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .replace(ranker)
        .map(|r| r.kind());
    match previous {
        Some(prev) if prev != kind => {
            log::info!("[tool-search] replaced ranker {prev} with {kind}")
        }
        Some(_) => log::debug!("[tool-search] re-installed ranker {kind}"),
        None => log::info!("[tool-search] installed ranker {kind}"),
    }
}

/// The process-wide ranker, if one was installed.
pub fn installed_tool_ranker() -> Option<Arc<dyn ToolRanker>> {
    ranker_slot()
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

/// Remove the process-wide ranker. Test hook.
pub fn clear_tool_ranker() {
    ranker_slot()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take();
}

/// Record the `agent.tool_search` settings every later turn should honour.
/// Called by the session builder, which has the config in hand; the turn
/// harness, which does not, reads them back through [`discovery_policy`].
pub fn apply_tool_search_config(config: &ToolSearchConfig) {
    let mut slot = settings_slot()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if *slot != *config {
        log::info!(
            "[tool-search] settings: ranker={} top_k={}",
            config.ranker,
            config.top_k
        );
        *slot = config.clone();
    }
}

/// The settings in force.
pub fn tool_search_config() -> ToolSearchConfig {
    settings_slot()
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

/// The discovery policy for a turn: the installed ranker, used as the
/// settings say, over the harness defaults.
///
/// `"auto"` and `"jev"` both serve the installed ranker (it answers with a
/// fallback of its own when the process has no credential); `"bm25"` ignores
/// it; `"compare"` serves it and records BM25 alongside. An unknown value is
/// treated as `"auto"` and logged once per turn.
pub(crate) fn discovery_policy() -> ToolDiscoveryPolicy {
    let settings = tool_search_config();
    let mut policy = ToolDiscoveryPolicy::default();
    policy.default_limit = settings.top_k.clamp(1, policy.max_limit);
    let mode = match settings.ranker.trim().to_ascii_lowercase().as_str() {
        "auto" | "jev" | "ranker" => DiscoveryRankMode::Ranker,
        "bm25" | "lexical" => DiscoveryRankMode::Bm25,
        "compare" | "both" => DiscoveryRankMode::Compare,
        other => {
            log::warn!("[tool-search] unknown ranker setting {other:?}; using auto");
            DiscoveryRankMode::Ranker
        }
    };
    policy.rank_mode = mode;
    policy.ranker = installed_tool_ranker();
    tracing::debug!(
        ranker = ?policy.ranker.as_ref().map(|r| r.kind()),
        mode = ?policy.rank_mode,
        top_k = policy.default_limit,
        "[tool-search] discovery policy for turn"
    );
    policy
}

/// The `tool_search` / `tool_call` bridge, rendered for a TEXT dialect's
/// prompt catalogue.
///
/// The harness mints these two schemas onto `request.tools` whenever a run
/// has a deferred catalogue, which is all a native-tool-calling provider
/// needs. A text dialect (P-Format / code) never sees that set: the dialect
/// folds the catalogue into the system prompt and clears `request.tools`,
/// and because OpenHuman sets `host_renders_tool_catalogue` the harness
/// appends nothing of its own. So on those dialects the bridge only reaches
/// the model if the host's own catalogue carries it — otherwise the model
/// reads "invoke a match with `tool_call`" in a search result naming a tool
/// no signature in its catalogue describes, and answers with intent instead
/// of a call.
///
/// Built from [`tinyagents_harness::tool::discover::bridge_schemas`] rather
/// than hand-written prose, so the signature the model reads is the one
/// admission accepts. `deferred` only sizes the placeholder catalogue the
/// manifest is rendered from; the per-tool manifest itself is deliberately
/// left out of the prompt, because listing every deferred tool there is the
/// cost deferral exists to avoid.
pub(crate) fn bridge_prompt_tools(deferred: usize) -> Vec<crate::agent::prompts::PromptTool<'static>>
{
    if deferred == 0 {
        return Vec::new();
    }
    use tinyagents_harness::tool::discover::{bridge_schemas, DeferredCatalog};
    let mut policy = discovery_policy();
    // A zero budget renders the manifest as a bare count instead of naming
    // every deferred tool — the prompt advertises that a search exists, not
    // what it would find.
    policy.manifest_token_budget = 0;
    bridge_schemas(&DeferredCatalog::build(Vec::new()), &policy)
        .into_iter()
        .map(|schema| {
            crate::agent::prompts::PromptTool::owned(
                schema.name,
                schema.description,
                schema.parameters.to_string(),
            )
        })
        .collect()
}

/// The verb-gated token-overlap ranker the Composio sub-agent narrows its
/// toolkit with (`tinyagents_harness::tool::select::rank_tools_by_prompt`),
/// behind the [`ToolRanker`] trait so it can be installed, compared, or
/// benchmarked like any other. Not installed by default — it is the
/// baseline the search was measured against, kept callable on purpose.
#[derive(Debug, Default, Clone, Copy)]
pub struct OverlapRanker;

impl OverlapRanker {
    /// The stable [`ToolRanker::kind`] of this ranker.
    pub const KIND: &'static str = "overlap";
}

#[async_trait::async_trait]
impl ToolRanker for OverlapRanker {
    fn kind(&self) -> &'static str {
        Self::KIND
    }

    async fn rank(
        &self,
        intent: &str,
        _context: &tinytools::RankContext,
        candidates: &[tinytools::RankCandidate],
        limit: usize,
    ) -> Result<Vec<tinytools::RankHit>, tinytools::RankError> {
        use tinyagents_harness::tool::{rank_tools_by_prompt, SelectableTool};
        if intent.trim().is_empty() {
            return Err(tinytools::RankError::InvalidInput {
                reason: "intent is empty".to_owned(),
            });
        }
        let selectable: Vec<SelectableTool<'_>> = candidates
            .iter()
            .map(|c| SelectableTool::new(&c.key, &c.summary))
            .collect();
        Ok(rank_tools_by_prompt(intent, &selectable, limit)
            .into_iter()
            .enumerate()
            .map(|(rank, i)| {
                tinytools::RankHit::new(candidates[i].key.clone(), 1.0 / (rank as f64 + 1.0))
            })
            .collect())
    }
}

mod embedding_ranker;

pub use embedding_ranker::EmbeddingToolRanker;

#[cfg(test)]
#[path = "discovery_tests.rs"]
mod tests;
