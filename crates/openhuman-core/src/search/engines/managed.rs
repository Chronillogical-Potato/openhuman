use crate::config::Config;
use crate::search::registry::SearchToolParams;
use std::sync::Arc;
use tinytools::Tool;

pub(crate) fn build(root_config: &Config, params: SearchToolParams) -> Vec<Box<dyn Tool>> {
    tracing::debug!(
        requested = %root_config.search.requested_engine_str(),
        "[search] active engine = managed (backend-proxied web_search)"
    );

    vec![Box::new(crate::search::WebSearchTool::new(
        crate::integrations::build_client(root_config),
        Some(Arc::new(root_config.clone())),
        params.max_results,
        params.timeout_secs,
    ))]
}
