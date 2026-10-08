//! Composio sources: connector records become `Document` items.
//!
//! A Composio source names a toolkit (`gmail`, `notion`, …). Syncing it runs
//! the connector module's sync for each active connection of that toolkit
//! (`integrations::composio::ops::run_sync_pass`), which pages the account and
//! hands back decoded records; each record is stored as a document with
//! `source = composio:<source id>`, `tags = [toolkit, "connection:<id>", …]`,
//! its URL and its upstream timestamp. The connection tag is what deleting a
//! connection with `clear_memory` forgets by. The same conversion backs `openhuman.composio_sync`,
//! which syncs one connection on demand.

use chrono::{TimeZone, Utc};
use tinyconnectors_bus::records::ConnectorRecord;
use tinymemory_api::{DocumentBody, MemoryMeta, SourceKind, SourceRef, StoreItem};

use crate::config::schema::MemorySourceConfig;
use crate::config::Config;
use crate::integrations::composio::ops::{
    active_connection_ids, run_sync_pass, SYNC_PASS_MAX_ITEMS,
};
use crate::memory::engine::BoundEngine;
use crate::memory::error::{MemoryError, MemoryResult};

/// Held across one [`store_records`] call (storing, finding the previous
/// versions and forgetting them) and across [`forget_connection`]. Two
/// passes over the same records (the on-demand and the scheduled sync)
/// would otherwise interleave, and one could forget an item the other has
/// just made current again; a disconnect could miss an item a pass stores
/// while it runs.
// ponytail: one lock for every connection, as passes are short and rare; a
// per-connection lock if syncs of many accounts must overlap.
static STORE: std::sync::LazyLock<tokio::sync::Mutex<()>> =
    std::sync::LazyLock::new(|| tokio::sync::Mutex::new(()));

/// Connections deleted in this process, per workspace. A sync pass that
/// read a connection's records before its disconnect must not store them
/// after it, so [`forget_connection`] records the connection (under
/// [`STORE`]) and [`store_records`] drops any later batch for it. Composio
/// never reuses a connection id, so an entry stays for the life of the
/// process.
static DISCONNECTED: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashSet<(std::path::PathBuf, String)>>,
> = std::sync::LazyLock::new(Default::default);

fn disconnected(
) -> std::sync::MutexGuard<'static, std::collections::HashSet<(std::path::PathBuf, String)>> {
    DISCONNECTED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Whether `connection_id` was disconnected in this process: its records
/// are no longer read or stored.
#[must_use]
pub fn is_disconnected(config: &Config, connection_id: &str) -> bool {
    disconnected().contains(&(config.workspace_dir.clone(), connection_id.to_string()))
}

/// Most connector passes one source sync runs per connection.
const MAX_PASSES_PER_CONNECTION: usize = 25;

/// The tag every item synced through a connection carries.
#[must_use]
pub fn connection_tag(connection_id: &str) -> String {
    format!("connection:{connection_id}")
}

/// The document one connector record stores, or `None` for an empty record.
#[must_use]
pub fn record_item(
    toolkit: &str,
    connection_id: &str,
    source_id: &str,
    record: &ConnectorRecord,
) -> Option<StoreItem> {
    if record.content.trim().is_empty() {
        return None;
    }
    let mut tags = vec![toolkit.to_ascii_lowercase(), connection_tag(connection_id)];
    for tag in &record.tags {
        if !tag.trim().is_empty() && !tags.contains(tag) {
            tags.push(tag.clone());
        }
    }
    let observed_at = record
        .updated_at_ms
        .and_then(|ms| Utc.timestamp_millis_opt(ms).single());
    Some(StoreItem::Document {
        title: Some(record.title.trim().to_string()).filter(|title| !title.is_empty()),
        body: DocumentBody::Text(record.content.clone()),
        mime: record.mime.clone(),
        meta: MemoryMeta {
            url: record.url.clone(),
            tags,
            observed_at,
            source: SourceRef {
                kind: SourceKind::Composio,
                id: Some(source_id.to_string()),
            },
            ..MemoryMeta::default()
        },
    })
}

/// Files every non-empty record into `layout`'s brain, under the toolkit's
/// brain source, and forgets the previous version of each record that
/// changed or came back empty upstream ([`super::versions`]). Returns how
/// many were stored.
pub async fn store_records(
    config: &Config,
    bound: &BoundEngine,
    toolkit: &str,
    connection_id: &str,
    source_id: &str,
    layout: &tinymemory_tools::MemoryLayout,
    records: &[ConnectorRecord],
) -> MemoryResult<u64> {
    let _serial = STORE.lock().await;
    if is_disconnected(config, connection_id) {
        tracing::debug!(
            connection_id = %connection_id,
            "[memory:sources] dropping records read before their connection was deleted"
        );
        return Ok(0);
    }
    let mut keys = Vec::with_capacity(records.len());
    let mut items = Vec::with_capacity(records.len());
    let mut emptied = Vec::new();
    for record in records {
        let key = super::versions::key(connection_id, &record.item_id);
        match record_item(toolkit, connection_id, source_id, record) {
            Some(item) => {
                keys.push(key);
                items.push(item);
            }
            None => emptied.push(key),
        }
    }
    if !items.is_empty() {
        // Before the write, so an item stored is never under an unrecorded
        // root.
        super::roots::record(
            &config.workspace_dir,
            connection_id,
            &layout.root().to_string(),
        )
        .map_err(|error| {
            MemoryError::Engine(format!(
                "recording the connection's memory root failed: {error}"
            ))
        })?;
    }
    let ids = super::sync::store_all_ids(
        config,
        bound,
        items,
        (
            crate::config::schema::MemorySourceKind::Composio,
            toolkit,
            source_id,
        ),
        layout,
    )
    .await?;
    let stored: Vec<(String, String)> = keys
        .into_iter()
        .zip(ids)
        .filter_map(|(key, id)| Some((key, id?)))
        .collect();
    let stale = super::versions::begin(&config.workspace_dir, &stored, &emptied);
    forget_stale(config, bound, toolkit, stale).await;
    Ok(stored.len() as u64)
}

/// Forgets the previous versions [`super::versions::begin`] handed back and
/// settles them. A failed forget leaves them pending, so the next sync
/// retries; it never fails the sync.
async fn forget_stale(config: &Config, bound: &BoundEngine, toolkit: &str, stale: Vec<String>) {
    if stale.is_empty() {
        return;
    }
    let ids = stale
        .iter()
        .cloned()
        .map(tinymemory_api::ItemId::new)
        .collect();
    match bound
        .engine
        .forget(tinymemory_api::ForgetTarget::Ids(ids))
        .await
    {
        Ok(report) => {
            tracing::debug!(
                toolkit = %toolkit,
                stale = stale.len(),
                forgotten = report.forgotten,
                "[memory:sources] forgot the previous versions of edited records"
            );
            super::versions::settle(&config.workspace_dir, &stale);
        }
        Err(error) => tracing::warn!(
            toolkit = %toolkit,
            stale = stale.len(),
            code = MemoryError::from(error).code(),
            "[memory:sources] forgetting previous versions failed; retrying next sync"
        ),
    }
}

/// Syncs every active connection of the source's toolkit.
pub async fn sync_toolkit(
    config: &Config,
    bound: &BoundEngine,
    source: &MemorySourceConfig,
) -> MemoryResult<u64> {
    let toolkit = source.target.as_str();
    let connections = active_connection_ids(config, toolkit)
        .await
        .map_err(MemoryError::Engine)?;
    if connections.is_empty() {
        return Err(MemoryError::invalid(format!(
            "no active {toolkit} connection; connect it first"
        )));
    }
    let mut stored = 0u64;
    for connection_id in connections {
        for _ in 0..MAX_PASSES_PER_CONNECTION {
            let pass = run_sync_pass(
                config,
                bound,
                toolkit,
                &connection_id,
                &source.id,
                "manual",
                SYNC_PASS_MAX_ITEMS,
            )
            .await
            .map_err(MemoryError::Engine)?;
            stored += pass.written;
            if let Some(failure) = pass.failure {
                return Err(MemoryError::Engine(failure));
            }
            if !pass.more_pending {
                break;
            }
        }
    }
    Ok(stored)
}

/// The memory source id a Composio sync of `toolkit` files its items under:
/// the configured `composio` source for the toolkit when there is one, else
/// `composio:<toolkit>`.
#[must_use]
pub fn source_id_for_toolkit(config: &Config, toolkit: &str) -> String {
    use crate::integrations::composio::tools::canonicalize_toolkit_slug;
    let toolkit = canonicalize_toolkit_slug(toolkit);
    config
        .memory
        .sources
        .iter()
        .find(|source| {
            source.kind == crate::config::schema::MemorySourceKind::Composio
                && canonicalize_toolkit_slug(&source.target) == toolkit
        })
        .map_or_else(|| format!("composio:{toolkit}"), |source| source.id.clone())
}

/// Forgets every item synced through `connection_id`. Memory off forgets
/// nothing and is not an error.
///
/// With the connection's `toolkit` known, only that toolkit's brain source
/// is read (`source:<toolkit>`): under the current root and under every
/// root the connection filed items under before ([`super::roots`]), so
/// items left under a root no longer configured go too. Without the
/// toolkit, or when those sources hold none of the connection's items
/// (stored before roots were recorded), the whole tree is searched, so a
/// connection's items are never left behind.
pub async fn forget_connection(
    config: &Config,
    connection_id: &str,
    toolkit: Option<&str>,
) -> MemoryResult<usize> {
    // A sync pass storing this connection's records finishes first, so none
    // of its items lands after the forget; any pass after it, even one that
    // read its records before, stores nothing (`DISCONNECTED`). Marked even
    // with memory off, so nothing is stored if memory comes back on.
    let _serial = STORE.lock().await;
    disconnected().insert((config.workspace_dir.clone(), connection_id.to_string()));
    let bound = match crate::memory::engine::resolve(config).engine() {
        Ok(bound) => bound,
        Err(MemoryError::Off(_)) => return Ok(0),
        Err(error) => return Err(error),
    };
    let recorded = super::roots::of(&config.workspace_dir, connection_id);
    let reaches = match toolkit {
        // An unreadable roots record cannot bound the search: search all.
        Some(toolkit) => match &recorded {
            Some(recorded) => toolkit_reaches(config, toolkit, recorded)?,
            None => Vec::new(),
        },
        None => {
            tracing::warn!(
                connection_id = %connection_id,
                "[memory:sources] toolkit unknown; forgetting the connection across all memory"
            );
            Vec::new()
        }
    };
    tracing::debug!(
        connection_id = %connection_id,
        roots = reaches.len(),
        "[memory:sources] forgetting a connection's items"
    );
    let filter = |reach| tinymemory_api::MetaFilter {
        reach,
        sources: vec![SourceKind::Composio],
        tags_any: vec![connection_tag(connection_id)],
        ..tinymemory_api::MetaFilter::default()
    };
    let mut forgotten = 0;
    for reach in &reaches {
        forgotten += bound
            .engine
            .forget(tinymemory_api::ForgetTarget::Filter(filter(Some(
                reach.clone(),
            ))))
            .await?
            .forgotten;
    }
    if forgotten == 0 {
        if !reaches.is_empty() {
            tracing::debug!(
                connection_id = %connection_id,
                "[memory:sources] nothing in the toolkit's sources; searching all memory"
            );
        }
        forgotten = bound
            .engine
            .forget(tinymemory_api::ForgetTarget::Filter(filter(None)))
            .await?
            .forgotten;
    }
    if let Some(recorded) = &recorded {
        super::roots::forget(&config.workspace_dir, connection_id, recorded);
    }
    super::versions::drop_connection(&config.workspace_dir, connection_id);
    Ok(forgotten)
}

/// `source:<toolkit>` under the current root and under every `recorded`
/// root.
fn toolkit_reaches(
    config: &Config,
    toolkit: &str,
    recorded: &std::collections::BTreeSet<String>,
) -> MemoryResult<Vec<tinymemory_api::Reach>> {
    let source = crate::memory::brain::brain_source(
        crate::config::schema::MemorySourceKind::Composio,
        toolkit,
    );
    let current = super::layout_of(config, &source_id_for_toolkit(config, toolkit));
    let mut roots = recorded.clone();
    roots.insert(current.root().to_string());
    let mut reaches = Vec::with_capacity(roots.len());
    for root in roots {
        let layout = root
            .parse::<tinymemory_api::Namespace>()
            .map_err(|error| error.to_string())
            .and_then(|root| {
                tinymemory_tools::MemoryLayout::new(root).map_err(|error| error.to_string())
            });
        match layout {
            Ok(layout) => reaches.push(tinymemory_api::Reach::subtree(layout.brain(&source)?)),
            Err(error) => {
                tracing::warn!(%error, "[memory:sources] skipping an unreadable recorded root")
            }
        }
    }
    Ok(reaches)
}

#[cfg(test)]
#[path = "composio_tests.rs"]
mod tests;
