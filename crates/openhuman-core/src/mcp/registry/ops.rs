//! RPC handler implementations for the MCP clients domain.
//!
//! Every function here maps one-to-one with a `schemas.rs` handler and keeps
//! the signature and the JSON shape it had before the client moved out to
//! `tinymcp` — the frontend calls these methods and nothing about its contract
//! changed. What changed is underneath: each one now delegates to the service
//! [`super::super::host`] holds.
//!
//! # What stayed here on purpose
//!
//! **Events.** `tinymcp` reports what happened in its return values and
//! publishes nothing. `DomainEvent` is this application's vocabulary, so the
//! publishing happens at this layer, where that vocabulary is known.
//!
//! **The scan over remote tool definitions.** Prompt-injection detection is
//! host policy: the detector, its rules, and what a hit means belong to this
//! application's threat model.
//!
//! **The `mcp.json` document.** Installing a server is no longer a catalog
//! action: the user declares servers in one `mcp.json` document and this layer
//! reconciles the store against it (`config_get` / `config_set`, in
//! [`super::config_doc`]). The catalog is browse-only.

use std::collections::HashMap;
use std::time::Instant;

use serde_json::{json, Value};

use crate::config::Config;
use crate::core::bus::BUS;
use crate::core::events::DomainEvent;
use crate::mcp::host;
use crate::rpc::RpcOutcome;

use super::config_doc;
use super::helpers::{encode, inject_required_env_keys, require, resolve};

// ── registry_search ──────────────────────────────────────────────────────────

/// `transport` is accepted and ignored: the catalog no longer filters by it,
/// because the picker chooses a connection at install time from what the server
/// actually offers. The parameter stays so the frontend's call does not have to
/// change in the same release.
pub async fn mcp_clients_registry_search(
    config: &Config,
    query: Option<String>,
    _transport: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
) -> Result<RpcOutcome<Value>, String> {
    let page = page.unwrap_or(1);
    let page_size = page_size.unwrap_or(20);

    let mut found = resolve(config)?
        .dynamic()
        .registry_search(query.as_deref(), page, page_size)
        .await
        .map_err(|error| error.to_string())?;

    // Badging and the strict filter are presentation choices, applied here so a
    // caller assembling its own view does not have to undo them.
    tinymcp::registry::curation::tag_official(&mut found.servers);
    tinymcp::registry::curation::float_official_first(&mut found.servers);

    let count = found.servers.len();
    Ok(RpcOutcome::new(
        json!({
            "servers": found.servers,
            "page": found.page,
            "total_pages": found.total_pages,
        }),
        vec![format!("registry_search returned {count} servers")],
    ))
}

// ── registry_get ─────────────────────────────────────────────────────────────

pub async fn mcp_clients_registry_get(
    config: &Config,
    qualified_name: String,
) -> Result<RpcOutcome<Value>, String> {
    let qualified_name = require(&qualified_name, "qualified_name")?;

    let (detail, required_env_keys) = resolve(config)?
        .dynamic()
        .registry_get(&qualified_name)
        .await
        .map_err(|error| error.to_string())?;

    // The registry tab shows what a server would ask for, and fetching the two
    // separately would be two catalog round trips for one screen.
    let mut server = encode(&detail)?;
    inject_required_env_keys(&mut server, &required_env_keys);

    Ok(RpcOutcome::new(
        json!({ "server": server }),
        vec![format!(
            "registry_get ok: {qualified_name} env_keys={}",
            required_env_keys.len()
        )],
    ))
}

// ── installed_list ───────────────────────────────────────────────────────────

pub async fn mcp_clients_installed_list(config: &Config) -> Result<RpcOutcome<Value>, String> {
    let installed = resolve(config)?
        .dynamic()
        .installed_list()
        .map_err(|error| error.to_string())?;

    let count = installed.len();
    Ok(RpcOutcome::new(
        json!({ "installed": installed }),
        vec![format!("installed_list returned {count} servers")],
    ))
}

// ── uninstall ────────────────────────────────────────────────────────────────

pub async fn mcp_clients_uninstall(
    config: &Config,
    server_id: String,
) -> Result<RpcOutcome<Value>, String> {
    let server_id = require(&server_id, "server_id")?;

    let removed = resolve(config)?
        .dynamic()
        .uninstall(&server_id)
        .await
        .map_err(|error| error.to_string())?;

    Ok(RpcOutcome::new(
        json!({ "server_id": server_id, "removed": removed }),
        vec![format!("uninstalled server_id={server_id}")],
    ))
}

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

// ── auth detection and browser OAuth ─────────────────────────────────────────

pub async fn mcp_clients_detect_auth(
    config: &Config,
    server_id: String,
) -> Result<RpcOutcome<Value>, String> {
    let server_id = require(&server_id, "server_id")?;

    let detection = resolve(config)?
        .dynamic()
        .detect_auth(&server_id)
        .await
        .map_err(|error| error.to_string())?;

    let kind = detection.kind.as_str();
    let value = encode(&detection)?;

    Ok(RpcOutcome::new(
        value,
        vec![format!("detect_auth {server_id} -> {kind}")],
    ))
}

pub async fn mcp_clients_oauth_begin(
    config: &Config,
    server_id: String,
) -> Result<RpcOutcome<Value>, String> {
    let server_id = require(&server_id, "server_id")?;

    let authorize_url = resolve(config)?
        .dynamic()
        .oauth_begin(&server_id, &host::oauth_redirect_uri())
        .await
        .map_err(|error| error.to_string())?;

    Ok(RpcOutcome::new(
        json!({ "authorize_url": authorize_url }),
        vec![format!("oauth_begin {server_id}")],
    ))
}

// ── connect ──────────────────────────────────────────────────────────────────

pub async fn mcp_clients_connect(
    config: &Config,
    server_id: String,
) -> Result<RpcOutcome<Value>, String> {
    let server_id = require(&server_id, "server_id")?;

    let outcome = resolve(config)?
        .dynamic()
        .connect(&server_id)
        .await
        .map_err(|error| error.to_string())?;

    let tools = super::tools_safe_for_agent(&server_id, outcome.tools);
    let tool_count = u32::try_from(tools.len()).unwrap_or(u32::MAX);

    BUS.publish(DomainEvent::McpServerConnected {
        server_id: server_id.clone(),
        tool_count,
    });

    Ok(RpcOutcome::new(
        json!({ "server_id": server_id, "status": "connected", "tools": tools }),
        vec![format!(
            "connected server_id={server_id} tools={tool_count}"
        )],
    ))
}

// ── set_enabled ──────────────────────────────────────────────────────────────

pub async fn mcp_clients_set_enabled(
    config: &Config,
    server_id: String,
    enabled: bool,
) -> Result<RpcOutcome<Value>, String> {
    let server_id = require(&server_id, "server_id")?;

    resolve(config)?
        .dynamic()
        .set_enabled(&server_id, enabled)
        .await
        .map_err(|error| error.to_string())?;

    if !enabled {
        BUS.publish(DomainEvent::McpServerDisconnected {
            server_id: server_id.clone(),
            reason: Some("disabled".to_string()),
        });
    }

    Ok(RpcOutcome::new(
        json!({ "server_id": server_id, "enabled": enabled }),
        vec![format!(
            "set_enabled server_id={server_id} enabled={enabled}"
        )],
    ))
}

// ── disconnect ───────────────────────────────────────────────────────────────

pub async fn mcp_clients_disconnect(
    config: &Config,
    server_id: String,
) -> Result<RpcOutcome<Value>, String> {
    let server_id = require(&server_id, "server_id")?;

    resolve(config)?
        .dynamic()
        .disconnect(&server_id)
        .await
        .map_err(|error| error.to_string())?;

    BUS.publish(DomainEvent::McpServerDisconnected {
        server_id: server_id.clone(),
        reason: None,
    });

    Ok(RpcOutcome::new(
        json!({ "server_id": server_id, "status": "disconnected" }),
        vec![format!("disconnected server_id={server_id}")],
    ))
}

// ── update_env ───────────────────────────────────────────────────────────────

pub async fn mcp_clients_update_env(
    config: &Config,
    server_id: String,
    env: HashMap<String, String>,
) -> Result<RpcOutcome<Value>, String> {
    use tinymcp_bus::UpdateEnvStatus;

    let server_id = require(&server_id, "server_id")?;

    let outcome = resolve(config)?
        .dynamic()
        .update_env(&server_id, env.into_iter().collect())
        .await
        .map_err(|error| error.to_string())?;

    match outcome.status {
        UpdateEnvStatus::Connected => {
            let tools = super::tools_safe_for_agent(&server_id, outcome.tools);
            let tool_count = u32::try_from(tools.len()).unwrap_or(u32::MAX);

            BUS.publish(DomainEvent::McpServerConnected {
                server_id: server_id.clone(),
                tool_count,
            });

            Ok(RpcOutcome::new(
                json!({
                    "server_id": server_id,
                    "status": "connected",
                    "env_keys": outcome.env_keys,
                    "tools": tools,
                }),
                vec![format!(
                    "update_env reconnected server_id={server_id} tools={tool_count}"
                )],
            ))
        }
        UpdateEnvStatus::Disabled => Ok(RpcOutcome::new(
            json!({
                "server_id": server_id,
                "status": "disabled",
                "env_keys": outcome.env_keys,
            }),
            vec![format!(
                "update_env persisted env for server_id={server_id} but did not reconnect: server is disabled"
            )],
        )),
        UpdateEnvStatus::Unauthorized => {
            // The reason code, never the raw 401 message: that leaks the OAuth
            // metadata URL, and the frontend renders localized copy from the
            // code alone.
            let hint = outcome.auth_hint.map(|hint| hint.as_code()).unwrap_or_default();
            Ok(RpcOutcome::new(
                json!({
                    "server_id": server_id,
                    "status": "unauthorized",
                    "env_keys": outcome.env_keys,
                    "auth_hint": hint,
                }),
                vec![format!(
                    "update_env persisted env for server_id={server_id} but reconnect was unauthorized: {hint}"
                )],
            ))
        }
        _ => {
            let error = outcome.error.unwrap_or_default();
            Ok(RpcOutcome::new(
                json!({
                    "server_id": server_id,
                    "status": "disconnected",
                    "env_keys": outcome.env_keys,
                    "error": error,
                }),
                vec![format!(
                    "update_env persisted env for server_id={server_id} but reconnect failed: {error}"
                )],
            ))
        }
    }
}

// ── registry settings ────────────────────────────────────────────────────────

pub async fn mcp_clients_registry_settings_get(
    config: &Config,
) -> Result<RpcOutcome<Value>, String> {
    let settings = resolve(config)?.dynamic().registry_settings();

    Ok(RpcOutcome::new(
        encode(&settings)?,
        vec!["registry_settings_get".to_string()],
    ))
}

/// Persists the credentials and tells the running service about them.
///
/// Both halves are needed: the file is what survives a restart, and the service
/// is what the next search actually uses.
pub async fn mcp_clients_registry_settings_set(
    config: &mut Config,
    smithery_api_key: Option<String>,
    mcp_official_base: Option<String>,
    mcp_official_token: Option<String>,
) -> Result<RpcOutcome<Value>, String> {
    /// A blank update clears the field; an absent one leaves it.
    fn apply(field: &mut Option<String>, update: Option<String>) {
        if let Some(value) = update {
            let trimmed = value.trim();
            *field = (!trimmed.is_empty()).then(|| trimmed.to_string());
        }
    }

    let auth = &mut config.mcp_client.registry_auth;
    apply(&mut auth.smithery_api_key, smithery_api_key.clone());
    apply(&mut auth.mcp_official_base, mcp_official_base.clone());
    apply(&mut auth.mcp_official_token, mcp_official_token.clone());

    config.save().await.map_err(|error| error.to_string())?;

    let settings = resolve(config)?.dynamic().set_registry_settings(
        smithery_api_key,
        mcp_official_base,
        mcp_official_token,
    );

    Ok(RpcOutcome::new(
        encode(&settings)?,
        vec!["registry_settings_set saved".to_string()],
    ))
}

// ── status ───────────────────────────────────────────────────────────────────

pub async fn mcp_clients_status(config: &Config) -> Result<RpcOutcome<Value>, String> {
    let statuses = resolve(config)?
        .dynamic()
        .status()
        .await
        .map_err(|error| error.to_string())?;

    let count = statuses.len();
    Ok(RpcOutcome::new(
        json!({ "servers": statuses }),
        vec![format!("status returned {count} servers")],
    ))
}

// ── list_tools ───────────────────────────────────────────────────────────────

pub async fn mcp_clients_list_tools(
    config: &Config,
    server_id: String,
) -> Result<RpcOutcome<Value>, String> {
    let server_id = require(&server_id, "server_id")?;

    /// What a caller has to do about it either way.
    fn connect_first(server_id: &str) -> String {
        format!("server_id={server_id} is not connected; connect it first via mcp_clients_connect")
    }

    let tools = resolve(config)?
        .dynamic()
        .list_tools(&server_id)
        .await
        .map_err(|error| {
            tracing::debug!("[mcp-client] list_tools ({server_id}) failed: {error}");
            connect_first(&server_id)
        })?;

    let tools = super::tools_safe_for_agent(&server_id, tools);
    let count = tools.len();

    Ok(RpcOutcome::new(
        json!({ "server_id": server_id, "tools": tools }),
        vec![format!(
            "list_tools server_id={server_id} returned {count} tools"
        )],
    ))
}

// ── tool_call ────────────────────────────────────────────────────────────────

pub async fn mcp_clients_tool_call(
    config: &Config,
    server_id: String,
    tool_name: String,
    arguments: Value,
) -> Result<RpcOutcome<Value>, String> {
    let server_id = require(&server_id, "server_id")?;
    let tool_name = require(&tool_name, "tool_name")?;

    let start = Instant::now();
    let result = resolve(config)?
        .dynamic()
        .tool_call(&server_id, &tool_name, arguments)
        .await;
    let elapsed_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);

    BUS.publish(DomainEvent::McpClientToolExecuted {
        server_id: server_id.clone(),
        tool_name: tool_name.clone(),
        success: result.is_ok(),
        elapsed_ms,
    });

    match result {
        Ok(outcome) => Ok(RpcOutcome::new(
            json!({ "result": outcome.result, "is_error": outcome.is_error }),
            vec![format!(
                "tool_call ok server_id={server_id} tool={tool_name} elapsed_ms={elapsed_ms}"
            )],
        )),
        Err(error) => Ok(RpcOutcome::new(
            json!({ "result": error.to_string(), "is_error": true }),
            vec![format!(
                "tool_call error server_id={server_id} tool={tool_name}: {error}"
            )],
        )),
    }
}

#[cfg(test)]
#[path = "ops_tests.rs"]
mod tests;
