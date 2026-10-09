//! This host's binding of the authoring-draft store to its workspace.
//!
//! Same shape and same reason as [`super::store`]: the storage lives in
//! `tinyflows_sqlite::drafts` and takes a directory; this supplies the
//! directory. Drafts land in `<workspace_dir>/flows/drafts/<id>.json`, or in
//! the storage backend's catalog when one is configured.

use super::store::{dir, documents, run};
use crate::config::Config;
use anyhow::Result;
use serde_json::Value;
use tinyflows_catalog::{DraftOrigin, FlowDraft};

/// Binds [`tinyflows_sqlite::drafts::create_draft`] to this host's catalog directory.
#[inline]
pub fn create_draft(
    config: &Config,
    flow_id: Option<String>,
    name: String,
    graph: Value,
    origin: DraftOrigin,
) -> Result<FlowDraft> {
    if let Some(docs) = documents(config)? {
        return run(async move { docs.create_draft(flow_id, name, graph, origin).await });
    }
    tinyflows_sqlite::drafts::create_draft(&dir(config), flow_id, name, graph, origin)
}

/// Binds [`tinyflows_sqlite::drafts::get_draft`] to this host's catalog directory.
#[inline]
pub fn get_draft(config: &Config, id: &str) -> Result<Option<FlowDraft>> {
    if let Some(docs) = documents(config)? {
        let id = id.to_string();
        return run(async move { docs.get_draft(&id).await });
    }
    tinyflows_sqlite::drafts::get_draft(&dir(config), id)
}

/// Binds [`tinyflows_sqlite::drafts::update_draft`] to this host's catalog directory.
#[inline]
pub fn update_draft(
    config: &Config,
    id: &str,
    name: Option<String>,
    graph: Option<Value>,
    flow_id: Option<Option<String>>,
) -> Result<FlowDraft> {
    if let Some(docs) = documents(config)? {
        let id = id.to_string();
        return run(async move { docs.update_draft(&id, name, graph, flow_id).await });
    }
    tinyflows_sqlite::drafts::update_draft(&dir(config), id, name, graph, flow_id)
}

/// Binds [`tinyflows_sqlite::drafts::list_drafts`] to this host's catalog directory.
#[inline]
pub fn list_drafts(config: &Config) -> Result<Vec<FlowDraft>> {
    if let Some(docs) = documents(config)? {
        return run(async move { docs.list_drafts().await });
    }
    tinyflows_sqlite::drafts::list_drafts(&dir(config))
}

/// Binds [`tinyflows_sqlite::drafts::delete_draft`] to this host's catalog directory.
#[inline]
pub fn delete_draft(config: &Config, id: &str) -> Result<bool> {
    if let Some(docs) = documents(config)? {
        let id = id.to_string();
        return run(async move { docs.delete_draft(&id).await });
    }
    tinyflows_sqlite::drafts::delete_draft(&dir(config), id)
}
