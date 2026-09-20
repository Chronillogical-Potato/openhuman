//! The user's MCP servers as one `mcp.json` document.
//!
//! Installing a server is not a catalog action any more. The registry tab is
//! browse-only; a user who found a server there reads its own install page and
//! declares it here, in the same `{ "mcpServers": { … } }` shape every desktop
//! MCP client already uses. This module is the contract for that document and
//! the reconciliation of the install store against it.
//!
//! # One store, two notations
//!
//! The rows the **Servers** tab renders and the document the **mcp.json** tab
//! edits are the same `tinymcp` store. [`render`] projects the store into the
//! document; [`parse`] reads a document back into declarations; and
//! [`config_ops::mcp_clients_config_set`](super::config_ops) applies the difference. Neither
//! is an import format that can drift from what is configured.
//!
//! # Credentials are write-only
//!
//! A stdio server's `env` and an HTTP server's `headers` are secrets. They are
//! accepted on write and stored in the credential table; a read never echoes a
//! value, only the names (`envKeys`) and whether anything is stored
//! (`authConfigured`). An entry saved *without* an `env`/`headers` block
//! therefore keeps its stored credentials — a round trip cannot silently
//! de-authenticate a server. A key set to the empty string removes that one
//! stored value.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use tinymcp_bus::{InstalledServer, Transport};

/// The document's root key.
pub const ROOT_KEY: &str = "mcpServers";

/// Fields a read echoes for the reader and a write ignores.
const ECHOED_FIELDS: &[&str] = &["envKeys", "authConfigured"];

/// Fields a write accepts without acting on, because other clients emit them.
const TOLERATED_FIELDS: &[&str] = &["type", "transport"];

/// Credential names beginning this way are the store's own bookkeeping (the
/// OAuth refresh bundle) and are neither rendered nor overwritten from here.
const INTERNAL_KEY_PREFIX: &str = "__";

/// One server as the document declares it, after validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declared {
    /// The document key; doubles as the store's `qualified_name`.
    pub name: String,
    /// How the server is dialled.
    pub transport: Transport,
    /// The launcher, for stdio; empty for HTTP.
    pub command: String,
    /// Arguments to the launcher, for stdio.
    pub args: Vec<String>,
    /// Credentials named in this write: stdio `env` or HTTP `headers`.
    ///
    /// `None` when the block was absent, which leaves stored values alone.
    pub credentials: Option<BTreeMap<String, String>>,
    /// Free text shown beside the row.
    pub description: Option<String>,
    /// Whether the server is brought up and exposed.
    pub enabled: bool,
}

/// Projects the install store into the document.
///
/// Servers are keyed by `qualified_name` and sorted, so the same store renders
/// the same text and a re-save is not an edit. `authConfigured` is derived from
/// the stored credential names the caller passes in, never from values.
#[must_use]
pub fn render(servers: &[InstalledServer], stored_keys: &BTreeMap<String, Vec<String>>) -> Value {
    let mut entries: Vec<&InstalledServer> = servers.iter().collect();
    entries.sort_by(|a, b| a.qualified_name.cmp(&b.qualified_name));

    let mut out = Map::new();
    for server in entries {
        let mut entry = Map::new();
        match &server.transport {
            Transport::HttpRemote { url } => {
                entry.insert("url".into(), json!(url));
            }
            _ => {
                entry.insert("command".into(), json!(server.command));
                if !server.args.is_empty() {
                    entry.insert("args".into(), json!(server.args));
                }
            }
        }
        if let Some(description) = server.description.as_deref().filter(|d| !d.is_empty()) {
            entry.insert("description".into(), json!(description));
        }
        if !server.enabled {
            entry.insert("enabled".into(), json!(false));
        }

        let keys: Vec<&String> = stored_keys
            .get(&server.server_id)
            .map(|keys| {
                keys.iter()
                    .filter(|key| !key.starts_with(INTERNAL_KEY_PREFIX))
                    .collect()
            })
            .unwrap_or_default();
        if !keys.is_empty() {
            entry.insert("envKeys".into(), json!(keys));
        }
        entry.insert("authConfigured".into(), json!(!keys.is_empty()));

        out.insert(server.qualified_name.clone(), Value::Object(entry));
    }

    json!({ ROOT_KEY: out })
}

/// Reads a document into declarations, refusing anything it cannot honour.
///
/// The message names the entry and the field, because the text is still on
/// the user's screen and "invalid document" would send them hunting.
///
/// # Errors
///
/// Returns a sentence for the first problem found: a missing or non-object
/// `mcpServers`, an entry that is not an object, an entry with neither or both
/// of `url` and `command`, a field of the wrong type, or a field this host does
/// not understand (a `cwd`, say, which the install store cannot carry).
pub fn parse(doc: &Value) -> Result<Vec<Declared>, String> {
    let Some(root) = doc.as_object() else {
        return Err(format!("mcp.json holds an object with an `{ROOT_KEY}` key"));
    };
    let Some(servers) = root.get(ROOT_KEY) else {
        return Err(format!("no `{ROOT_KEY}` key — every server lives under it"));
    };
    let Some(servers) = servers.as_object() else {
        return Err(format!("`{ROOT_KEY}` maps a server name to its settings"));
    };

    let mut declared = Vec::with_capacity(servers.len());
    for (name, entry) in servers {
        let name = name.trim();
        if name.is_empty() {
            return Err("a server needs a name — one entry's key is empty".to_string());
        }
        let Some(entry) = entry.as_object() else {
            return Err(format!(
                "`{name}` holds an object, e.g. {{ \"command\": \"npx\", \"args\": [\"-y\", \"…\"] }} or {{ \"url\": \"https://…\" }}"
            ));
        };
        declared.push(parse_entry(name, entry)?);
    }
    Ok(declared)
}

/// Reads one entry.
fn parse_entry(name: &str, entry: &Map<String, Value>) -> Result<Declared, String> {
    for key in entry.keys() {
        let known = matches!(
            key.as_str(),
            "url" | "command" | "args" | "env" | "headers" | "description" | "enabled"
        ) || ECHOED_FIELDS.contains(&key.as_str())
            || TOLERATED_FIELDS.contains(&key.as_str());
        if !known {
            return Err(format!(
                "`{name}` has a `{key}` field this host doesn't understand; it accepts url, headers, command, args, env, description and enabled"
            ));
        }
    }

    let url = optional_string(name, entry, "url")?;
    let command = optional_string(name, entry, "command")?;

    let (transport, command) = match (url, command) {
        (Some(_), Some(_)) => {
            return Err(format!(
                "`{name}` names both a `url` and a `command`; a server is dialled one way"
            ));
        }
        (Some(url), None) => (Transport::HttpRemote { url }, String::new()),
        (None, Some(command)) => (Transport::Stdio, command),
        (None, None) => {
            return Err(format!(
                "`{name}` needs a `url` (hosted) or a `command` (run locally)"
            ));
        }
    };

    let args = match entry.get("args") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| format!("`{name}`.args holds strings only"))
            })
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => return Err(format!("`{name}`.args is a list of strings")),
    };
    if matches!(transport, Transport::HttpRemote { .. }) && !args.is_empty() {
        return Err(format!(
            "`{name}` is a hosted server; `args` only apply to a `command`"
        ));
    }

    // `env` is the stdio spelling and `headers` the HTTP one; both land in the
    // same credential table, because that is what the transport reads.
    let credential_field = match transport {
        Transport::Stdio => "env",
        _ => "headers",
    };
    let wrong_field = match transport {
        Transport::Stdio => "headers",
        _ => "env",
    };
    if entry.contains_key(wrong_field) {
        return Err(format!(
            "`{name}` takes `{credential_field}`, not `{wrong_field}`, for a `{}` server",
            match transport {
                Transport::Stdio => "command",
                _ => "url",
            }
        ));
    }
    let credentials = match entry.get(credential_field) {
        None | Some(Value::Null) => None,
        Some(Value::Object(values)) => {
            let mut out = BTreeMap::new();
            for (key, value) in values {
                let key = key.trim();
                if key.is_empty() {
                    return Err(format!("`{name}`.{credential_field} has an empty key"));
                }
                if key.starts_with(INTERNAL_KEY_PREFIX) {
                    return Err(format!(
                        "`{name}`.{credential_field}.{key} — names beginning `{INTERNAL_KEY_PREFIX}` are reserved"
                    ));
                }
                let Some(value) = value.as_str() else {
                    return Err(format!("`{name}`.{credential_field}.{key} holds a string"));
                };
                out.insert(key.to_string(), value.to_string());
            }
            Some(out)
        }
        Some(_) => {
            return Err(format!(
                "`{name}`.{credential_field} maps a name to a string value"
            ))
        }
    };

    let description = optional_string(name, entry, "description")?;
    let enabled = match entry.get("enabled") {
        None | Some(Value::Null) => true,
        Some(Value::Bool(flag)) => *flag,
        Some(_) => return Err(format!("`{name}`.enabled is true or false")),
    };

    Ok(Declared {
        name: name.to_string(),
        transport,
        command,
        args,
        credentials,
        description,
        enabled,
    })
}

/// A string field that may be absent, refusing a blank or non-string one.
fn optional_string(
    name: &str,
    entry: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, String> {
    match entry.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(format!("`{name}`.{key} is empty"));
            }
            Ok(Some(trimmed.to_string()))
        }
        Some(_) => Err(format!("`{name}`.{key} holds a string")),
    }
}

/// Whether a declaration describes the same dial as an install row.
///
/// Credentials are not compared: they are write-only, so a declaration cannot
/// know what is stored, and a re-save must not read as a change.
#[must_use]
pub fn same_dial(declared: &Declared, server: &InstalledServer) -> bool {
    declared.transport == server.transport
        && declared.command == server.command
        && declared.args == server.args
        && declared.description.as_deref().unwrap_or_default()
            == server.description.as_deref().unwrap_or_default()
        && declared.enabled == server.enabled
}

/// Builds the install row a declaration becomes.
///
/// `server_id` is the caller's: a fresh id for a new server, the existing one
/// for a server being rewritten in place, so the row the frontend and the
/// connection map address stays the same row.
#[must_use]
pub fn to_installed(declared: &Declared, server_id: String, installed_at: i64) -> InstalledServer {
    let command_kind = tinymcp::transport::stdio::spawn_env::required_runtime(&declared.command);
    InstalledServer {
        server_id,
        qualified_name: declared.name.clone(),
        display_name: declared.name.clone(),
        description: declared.description.clone(),
        icon_url: None,
        command_kind,
        command: declared.command.clone(),
        args: declared.args.clone(),
        env_keys: Vec::new(),
        config: None,
        installed_at,
        last_connected_at: None,
        transport: declared.transport.clone(),
        enabled: declared.enabled,
    }
}

/// Merges a write's credentials over what is stored.
///
/// A non-empty value replaces; an empty string removes; a name the write did
/// not mention is kept. Internal bookkeeping is never touched.
#[must_use]
pub fn merge_credentials(
    stored: &BTreeMap<String, String>,
    written: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut merged = stored.clone();
    for (key, value) in written {
        if value.is_empty() {
            merged.remove(key);
        } else {
            merged.insert(key.clone(), value.clone());
        }
    }
    merged
}

#[cfg(test)]
#[path = "config_doc_tests.rs"]
mod tests;
