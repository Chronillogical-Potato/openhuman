//! Wire-format types for `openhuman.skill_registry_*` RPC methods.

use serde::{Deserialize, Serialize};

use crate::core::ControllerSchema;
use crate::skills::catalog::types::{CatalogPage, CatalogQuery, Facet, Freshness};
use crate::skills::ops_types::WorkflowScope;

// ── Params ──────────────────────────────────────────────────────────────────

/// Filters and paging shared by `browse` and `search`.
#[derive(Debug, Deserialize, Default)]
pub(super) struct CatalogParams {
    #[serde(default)]
    pub(super) query: String,
    #[serde(default)]
    pub(super) source: Option<String>,
    #[serde(default)]
    pub(super) sources: Vec<String>,
    #[serde(default)]
    pub(super) category: Option<String>,
    #[serde(default)]
    pub(super) categories: Vec<String>,
    #[serde(default)]
    pub(super) page: Option<usize>,
    #[serde(default)]
    pub(super) page_size: Option<usize>,
    #[serde(default)]
    pub(super) force_refresh: bool,
}

fn merge_filter(single: Option<String>, many: Vec<String>) -> Vec<String> {
    let mut merged: Vec<String> = Vec::new();
    for value in single.into_iter().chain(many) {
        let value = value.trim().to_owned();
        if !value.is_empty() && !merged.iter().any(|seen| seen.eq_ignore_ascii_case(&value)) {
            merged.push(value);
        }
    }
    merged
}

impl CatalogParams {
    pub(super) fn into_query(self) -> CatalogQuery {
        CatalogQuery {
            text: self.query,
            upstreams: merge_filter(self.source, self.sources),
            categories: merge_filter(self.category, self.categories),
            page: self.page,
            page_size: self.page_size,
            force_refresh: self.force_refresh,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct EntryParams {
    pub(super) entry_id: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct UninstallParams {
    pub(super) name: String,
}

// ── Results ─────────────────────────────────────────────────────────────────

pub(super) type CatalogResult = CatalogPage;

#[derive(Debug, Serialize)]
pub(super) struct SourcesResult {
    pub(super) sources: Vec<String>,
    pub(super) facets: Vec<Facet>,
    pub(super) freshness: Freshness,
}

#[derive(Debug, Serialize)]
pub(super) struct CategoriesResult {
    pub(super) categories: Vec<String>,
    pub(super) facets: Vec<Facet>,
    pub(super) freshness: Freshness,
}

#[derive(Debug, Serialize)]
pub(super) struct InstallResult {
    pub(super) url: String,
    pub(super) stdout: String,
    pub(super) stderr: String,
    pub(super) new_skills: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct UninstallResult {
    pub(super) name: String,
    pub(super) removed_path: String,
    pub(super) scope: WorkflowScope,
}

#[derive(Debug, Serialize)]
pub(super) struct SchemasResult {
    pub(super) schemas: Vec<ControllerSchema>,
}

#[cfg(test)]
#[path = "wire_types_tests.rs"]
mod tests;
