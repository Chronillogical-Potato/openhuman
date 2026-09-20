//! The `mcp.json` RPC handlers: reading the install store as one document and
//! replacing it from one.
//!
//! The contract — what a read shows, what a write may say — is
//! [`super::config_doc`]; this file is the reconciliation against the store
//! and the events it publishes. Split from `ops.rs` so each stays a
//! readable size.

use serde_json::{json, Value};

use crate::config::Config;
use crate::core::bus::BUS;
use crate::core::events::DomainEvent;
use crate::rpc::RpcOutcome;

use super::config_doc;
use super::helpers::resolve;

// ── mcp.json ─────────────────────────────────────────────────────────────────

/// Renders the install store as the `mcp.json` document.
///
/// Credential *names* ride along as `envKeys`; values never do. See
/// [`super::config_doc`] for the contract.
pub async fn mcp_clients_config_get(config: &Config) -> Result<RpcOutcome<Value>, String> {
    let service = resolve(config)?;
    let installed = service
        .dynamic()
        .installed_list()
        .map_err(|error| error.to_string())?;

    let stored_keys = stored_credential_names(service.dynamic().store(), &installed);
    let count = installed.len();
    let doc = config_doc::render(&installed, &stored_keys);

    Ok(RpcOutcome::new(
        doc,
        vec![format!("config_get rendered {count} servers")],
    ))
}

/// Replaces the install store with what `doc` declares.
///
/// A replace, not a merge: a server absent from the document is uninstalled,
/// one not yet installed is added, and one whose dial changed is rewritten in
/// place under the same `server_id`. Credentials are the exception — a
/// declaration that says nothing about them leaves the stored ones alone,
/// because a read never shows them and a round trip must not wipe them.
///
/// Every added or rewritten server that is enabled is connected in the
/// background; the status poll reports how that went. The reply carries the
/// re-rendered document and what changed, so the editor can show both.
pub async fn mcp_clients_config_set(
    config: &Config,
    doc: Value,
) -> Result<RpcOutcome<Value>, String> {
    let declared = config_doc::parse(&doc)?;

    // Keys are trimmed on the way in, so two spellings of one name can collide
    // after the fact even though a JSON object cannot repeat a key.
    {
        let mut seen = std::collections::HashSet::new();
        for entry in &declared {
            if !seen.insert(entry.name.as_str()) {
                return Err(format!("`{}` is declared twice", entry.name));
            }
        }
    }

    let service = resolve(config)?;
    let registry = service.dynamic();
    let store = registry.store();
    let installed = registry
        .installed_list()
        .map_err(|error| error.to_string())?;

    let mut added = Vec::new();
    let mut updated = Vec::new();
    let mut removed = Vec::new();
    let mut to_connect: Vec<String> = Vec::new();

    // Removals first, so a server renamed in the document (drop one key, add
    // another) does not briefly hold two live connections to one process.
    for server in &installed {
        if declared
            .iter()
            .any(|entry| entry.name == server.qualified_name)
        {
            continue;
        }
        tracing::debug!(
            server_id = %server.server_id,
            name = %server.qualified_name,
            "[mcp] config_set removing a server absent from the document"
        );
        registry
            .uninstall(&server.server_id)
            .await
            .map_err(|error| error.to_string())?;
        BUS.publish(DomainEvent::McpServerDisconnected {
            server_id: server.server_id.clone(),
            reason: Some("removed from mcp.json".to_string()),
        });
        removed.push(server.qualified_name.clone());
    }

    for entry in &declared {
        let existing = installed
            .iter()
            .find(|server| server.qualified_name == entry.name);

        match existing {
            None => {
                let server_id = uuid::Uuid::new_v4().to_string();
                let row = config_doc::to_installed(entry, server_id.clone(), now_ms());
                store
                    .insert_server(&row)
                    .map_err(|error| error.to_string())?;
                if let Some(written) = &entry.credentials {
                    let merged = config_doc::merge_credentials(&Default::default(), written);
                    write_credentials(store, &server_id, &merged)?;
                }
                tracing::debug!(
                    server_id = %server_id,
                    name = %entry.name,
                    transport = entry.transport.dispatch_kind(),
                    "[mcp] config_set added a server"
                );
                BUS.publish(DomainEvent::McpServerInstalled {
                    server_id: server_id.clone(),
                    qualified_name: entry.name.clone(),
                });
                added.push(entry.name.clone());
                if entry.enabled {
                    to_connect.push(server_id);
                }
            }
            Some(existing) => {
                let server_id = existing.server_id.clone();
                let dial_changed = !config_doc::same_dial(entry, existing);
                let mut stored = store
                    .load_env_values(&server_id)
                    .map_err(|error| error.to_string())?;

                if dial_changed {
                    // The store has no "update the dial" statement, so the row
                    // is rewritten under the same id. Deleting it cascades the
                    // credentials, which is why they were read first.
                    registry.connections().disconnect(&server_id).await;
                    store
                        .delete_server(&server_id)
                        .map_err(|error| error.to_string())?;
                    let row = config_doc::to_installed(
                        entry,
                        server_id.clone(),
                        existing.installed_at,
                    );
                    store
                        .insert_server(&row)
                        .map_err(|error| error.to_string())?;
                    write_credentials(store, &server_id, &stored)?;
                    tracing::debug!(
                        server_id = %server_id,
                        name = %entry.name,
                        "[mcp] config_set rewrote a server whose dial changed"
                    );
                }

                let credentials_changed = match &entry.credentials {
                    Some(written) if !written.is_empty() => {
                        let merged = config_doc::merge_credentials(&stored, written);
                        let changed = merged != stored;
                        if changed {
                            write_credentials(store, &server_id, &merged)?;
                            stored = merged;
                        }
                        changed
                    }
                    _ => false,
                };
                drop(stored);

                if dial_changed || credentials_changed {
                    updated.push(entry.name.clone());
                    if entry.enabled {
                        if credentials_changed && !dial_changed {
                            // A new credential only takes effect on a fresh
                            // session.
                            registry.connections().disconnect(&server_id).await;
                        }
                        to_connect.push(server_id);
                    }
                }
            }
        }
    }

    // Connecting can take the transport's whole timeout per server; the editor
    // must not wait on it. The status poll shows each attempt's outcome, and a
    // failure is recorded against the server either way.
    for server_id in to_connect {
        let service = std::sync::Arc::clone(&service);
        tokio::spawn(async move {
            match service.dynamic().connect(&server_id).await {
                Ok(outcome) => {
                    let tool_count = u32::try_from(outcome.tools.len()).unwrap_or(u32::MAX);
                    tracing::debug!(
                        server_id = %server_id,
                        tools = tool_count,
                        "[mcp] config_set connected a server"
                    );
                    BUS.publish(DomainEvent::McpServerConnected {
                        server_id,
                        tool_count,
                    });
                }
                Err(error) => {
                    tracing::warn!(
                        server_id = %server_id,
                        "[mcp] config_set could not connect a server: {error}"
                    );
                }
            }
        });
    }

    let installed = registry
        .installed_list()
        .map_err(|error| error.to_string())?;
    let stored_keys = stored_credential_names(store, &installed);
    let mut rendered = config_doc::render(&installed, &stored_keys);

    let note = format!(
        "config_set added={} updated={} removed={}",
        added.len(),
        updated.len(),
        removed.len()
    );
    if let Some(object) = rendered.as_object_mut() {
        object.insert("added".into(), json!(added));
        object.insert("updated".into(), json!(updated));
        object.insert("removed".into(), json!(removed));
    }

    Ok(RpcOutcome::new(rendered, vec![note]))
}

/// The credential *names* stored for each server, keyed by `server_id`.
///
/// Values are loaded and dropped here; only the names leave this function.
fn stored_credential_names(
    store: &tinymcp::Store,
    installed: &[tinymcp_bus::InstalledServer],
) -> std::collections::BTreeMap<String, Vec<String>> {
    installed
        .iter()
        .map(|server| {
            let names = store
                .load_env_values(&server.server_id)
                .map(|values| values.into_keys().collect())
                .unwrap_or_default();
            (server.server_id.clone(), names)
        })
        .collect()
}

/// Stores a server's credentials and keeps the row's name list in step.
fn write_credentials(
    store: &tinymcp::Store,
    server_id: &str,
    values: &std::collections::BTreeMap<String, String>,
) -> Result<(), String> {
    store
        .set_env_values(server_id, values)
        .map_err(|error| error.to_string())?;
    let names: Vec<String> = values.keys().cloned().collect();
    store
        .update_env_keys(server_id, &names)
        .map_err(|error| error.to_string())
}

/// Now, in Unix epoch milliseconds, as the install rows record it.
fn now_ms() -> i64 {
    i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis())
            .unwrap_or_default(),
    )
    .unwrap_or(i64::MAX)
}
