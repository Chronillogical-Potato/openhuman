use crate::config::Config;
use crate::search::registry::SearchToolParams;
use tinytools::Tool;

pub(crate) fn build(_: &Config, _: SearchToolParams) -> Vec<Box<dyn Tool>> {
    tracing::debug!("[search] disabled — no search tools registered");
    Vec::new()
}
