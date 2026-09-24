//! Agent-facing search tool implementations, one module per provider.
//!
//! See `README.md` for the provider -> tool -> transport table and how each
//! family is registered (engine registry, `IntegrationClient`, or a direct
//! RPC handler).

mod brave;
mod exa;
mod parallel;
mod querit;
mod searxng;
mod seltz;
mod tavily;
mod tinyfish;
mod web_search;

pub use brave::{
    BraveImageSearchTool, BraveNewsSearchTool, BraveVideoSearchTool, BraveWebSearchTool,
};
pub use exa::{
    ExaFindSimilarTool, ExaGetContentsTool, ExaResultItem, ExaSearchResponse, ExaSearchTool,
};
pub use parallel::{
    ParallelChatTool, ParallelDatasetTool, ParallelEnrichTool, ParallelExtractTool,
    ParallelResearchTool, ParallelSearchTool, SearchResponse, SearchResultItem,
};
pub use querit::QueritSearchTool;
pub use searxng::{
    normalize_categories, SearxngSearchArgs, SearxngSearchResponse, SearxngSearchTool,
    MAX_RESULTS as SEARXNG_MAX_RESULTS,
};
pub use seltz::SeltzSearchTool;
pub use tavily::{
    TavilyExtractResponse, TavilyExtractResult, TavilyExtractTool, TavilyImage, TavilyResultItem,
    TavilySearchResponse, TavilySearchTool,
};
pub use tinyfish::{TinyFishAgentRunTool, TinyFishFetchTool, TinyFishSearchTool};
pub use web_search::WebSearchTool;
// Crate-internal: the `tools.web_search` RPC reuses the same provider
// resolution so both managed-search surfaces attribute a call identically.
pub(crate) use web_search::resolve_managed_provider;

/// Maximum characters kept from a web-search result's excerpt when it is
/// copied into [`ToolResult::metadata`][tinytools::ToolResult] for the UI
/// (issue: tool-call presentation). Deliberately smaller than the ~500-char
/// budget the model-facing text renders with — this metadata is for a compact
/// result card, not the full context the model reads.
pub(crate) const WEB_SEARCH_METADATA_EXCERPT_CHARS: usize = 300;

/// One search result, borrowed from whatever provider-specific struct the
/// caller already has, for building the structured
/// `{"kind":"web_search",...}` metadata every model-facing web-search tool
/// attaches to its [`tinytools::ToolResult::metadata`] (issue: tool-call
/// presentation). Kept separate from the model-facing rendering so a change
/// here can never perturb the byte-identical prompt text the provider's cache
/// keys on.
pub(crate) struct WebSearchResultRef<'a> {
    pub title: &'a str,
    pub url: &'a str,
    pub published: Option<&'a str>,
    pub excerpt: Option<&'a str>,
}

/// Build the structured web-search metadata payload:
/// `{"kind":"web_search","query":...,"provider":...,"results":[{"title":...,
/// "url":...,"published":...?,"excerpt":...?}]}`. `max_results` caps how many
/// of `results` are copied in, matching whatever cap the tool's own
/// model-facing rendering already applies so the structured payload never
/// claims more results exist than the model was shown.
///
/// This is metadata (host-only, never rendered to the model) — see
/// [`tinytools::ToolResult::metadata`]'s own docs on that boundary.
pub(crate) fn web_search_metadata(
    query: &str,
    provider: &str,
    results: &[WebSearchResultRef<'_>],
    max_results: usize,
) -> serde_json::Value {
    let results_json: Vec<serde_json::Value> = results
        .iter()
        .take(max_results)
        .map(|r| {
            let mut obj = serde_json::json!({
                "title": r.title,
                "url": r.url,
            });
            if let Some(published) = r.published.map(str::trim).filter(|s| !s.is_empty()) {
                obj["published"] = serde_json::json!(published);
            }
            if let Some(excerpt) = r.excerpt.map(str::trim).filter(|s| !s.is_empty()) {
                obj["excerpt"] = serde_json::json!(crate::util::truncate_with_ellipsis(
                    excerpt,
                    WEB_SEARCH_METADATA_EXCERPT_CHARS
                ));
            }
            obj
        })
        .collect();
    serde_json::json!({
        "kind": "web_search",
        "query": query,
        "provider": provider,
        "results": results_json,
    })
}
