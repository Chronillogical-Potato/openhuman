//! The `mcp.json` contract: what a read shows, what a write accepts, and what
//! counts as a change.

use std::collections::BTreeMap;

use serde_json::json;
use tinymcp_bus::{CommandKind, InstalledServer, Transport};

use super::*;

fn stdio_row(name: &str) -> InstalledServer {
    InstalledServer {
        server_id: format!("id-{name}"),
        qualified_name: name.to_string(),
        display_name: name.to_string(),
        description: None,
        icon_url: None,
        command_kind: CommandKind::Node,
        command: "npx".to_string(),
        args: vec!["-y".to_string(), "@x/y".to_string()],
        env_keys: vec![],
        config: None,
        installed_at: 1,
        last_connected_at: None,
        transport: Transport::Stdio,
        enabled: true,
    }
}

fn http_row(name: &str) -> InstalledServer {
    InstalledServer {
        transport: Transport::HttpRemote {
            url: "https://x.test/mcp".to_string(),
        },
        command: String::new(),
        args: vec![],
        ..stdio_row(name)
    }
}

// ── render ───────────────────────────────────────────────────────────────────

#[test]
fn a_read_shows_the_dial_and_never_a_credential_value() {
    let mut keys = BTreeMap::new();
    keys.insert(
        "id-a".to_string(),
        vec!["API_KEY".to_string(), "__oauth".to_string()],
    );
    let doc = render(&[stdio_row("a"), http_row("b")], &keys);

    assert_eq!(
        doc,
        json!({
            "mcpServers": {
                "a": {
                    "command": "npx",
                    "args": ["-y", "@x/y"],
                    "envKeys": ["API_KEY"],
                    "authConfigured": true,
                },
                "b": {
                    "url": "https://x.test/mcp",
                    "authConfigured": false,
                }
            }
        })
    );
}

#[test]
fn a_read_is_sorted_and_says_when_a_server_is_off() {
    let mut off = stdio_row("zeta");
    off.enabled = false;
    off.description = Some("the last one".to_string());
    let doc = render(&[off, stdio_row("alpha")], &BTreeMap::new());
    let names: Vec<&String> = doc["mcpServers"].as_object().unwrap().keys().collect();
    assert_eq!(names, ["alpha", "zeta"]);
    assert_eq!(doc["mcpServers"]["zeta"]["enabled"], json!(false));
    assert_eq!(
        doc["mcpServers"]["zeta"]["description"],
        json!("the last one")
    );
    assert!(doc["mcpServers"]["alpha"].get("enabled").is_none());
}

// ── parse ────────────────────────────────────────────────────────────────────

#[test]
fn a_write_reads_both_spellings() {
    let declared = parse(&json!({
        "mcpServers": {
            "local": { "command": "uvx", "args": ["thing"], "env": { "TOKEN": "t" } },
            "hosted": { "type": "http", "url": "https://h.test/mcp", "headers": { "Authorization": "Bearer x" }, "enabled": false },
        }
    }))
    .unwrap();

    let local = declared.iter().find(|d| d.name == "local").unwrap();
    assert_eq!(local.transport, Transport::Stdio);
    assert_eq!(local.command, "uvx");
    assert_eq!(local.args, ["thing"]);
    assert_eq!(
        local.credentials.as_ref().unwrap().get("TOKEN").unwrap(),
        "t"
    );
    assert!(local.enabled);

    let hosted = declared.iter().find(|d| d.name == "hosted").unwrap();
    assert_eq!(
        hosted.transport,
        Transport::HttpRemote {
            url: "https://h.test/mcp".to_string()
        }
    );
    assert!(!hosted.enabled);
    assert_eq!(
        hosted
            .credentials
            .as_ref()
            .unwrap()
            .get("Authorization")
            .unwrap(),
        "Bearer x"
    );
}

#[test]
fn a_write_without_a_credential_block_leaves_it_unsaid() {
    let declared = parse(&json!({ "mcpServers": { "a": { "command": "npx" } } })).unwrap();
    assert_eq!(declared[0].credentials, None);
}

#[test]
fn the_echoed_fields_are_ignored_on_write() {
    let declared = parse(&json!({
        "mcpServers": { "a": { "url": "https://a.test", "envKeys": ["X"], "authConfigured": true } }
    }))
    .unwrap();
    assert_eq!(declared.len(), 1);
    assert_eq!(declared[0].credentials, None);
}

#[test]
fn refusals_name_the_entry_and_the_field() {
    let cases: Vec<(serde_json::Value, &str)> = vec![
        (json!([]), "holds an object"),
        (json!({}), "no `mcpServers` key"),
        (json!({ "mcpServers": 1 }), "maps a server name"),
        (
            json!({ "mcpServers": { "": {} } }),
            "one entry's key is empty",
        ),
        (json!({ "mcpServers": { "a": "x" } }), "`a` holds an object"),
        (json!({ "mcpServers": { "a": {} } }), "`a` needs a `url`"),
        (
            json!({ "mcpServers": { "a": { "url": "u", "command": "c" } } }),
            "both a `url` and a `command`",
        ),
        (
            json!({ "mcpServers": { "a": { "command": "c", "cwd": "/x" } } }),
            "`cwd` field this host doesn't understand",
        ),
        (
            json!({ "mcpServers": { "a": { "url": "u", "args": ["x"] } } }),
            "`args` only apply to a `command`",
        ),
        (
            json!({ "mcpServers": { "a": { "url": "u", "env": {} } } }),
            "takes `headers`, not `env`",
        ),
        (
            json!({ "mcpServers": { "a": { "command": "c", "env": { "__x": "1" } } } }),
            "reserved",
        ),
        (
            json!({ "mcpServers": { "a": { "command": "c", "env": { "K": 1 } } } }),
            "`a`.env.K holds a string",
        ),
        (
            json!({ "mcpServers": { "a": { "command": "c", "enabled": "yes" } } }),
            "`a`.enabled is true or false",
        ),
        (
            json!({ "mcpServers": { "a": { "command": "   " } } }),
            "`a`.command is empty",
        ),
    ];
    for (doc, expected) in cases {
        let error = parse(&doc).expect_err("refused");
        assert!(
            error.contains(expected),
            "{doc}: expected {expected:?} in {error:?}"
        );
    }
}

// ── reconciliation helpers ───────────────────────────────────────────────────

#[test]
fn a_re_save_of_what_is_installed_is_not_a_change() {
    let row = stdio_row("a");
    let declared = parse(&render(&[row.clone()], &BTreeMap::new())).unwrap();
    assert!(same_dial(&declared[0], &row));
}

#[test]
fn a_changed_argument_or_flag_is_a_change() {
    let row = stdio_row("a");
    let mut declared = parse(&render(&[row.clone()], &BTreeMap::new())).unwrap();
    declared[0].args.push("--verbose".to_string());
    assert!(!same_dial(&declared[0], &row));

    let mut declared = parse(&render(&[row.clone()], &BTreeMap::new())).unwrap();
    declared[0].enabled = false;
    assert!(!same_dial(&declared[0], &row));
}

#[test]
fn a_declaration_becomes_a_row_keyed_by_its_name() {
    let declared =
        parse(&json!({ "mcpServers": { "fs": { "command": "npx", "args": ["-y", "x"] } } }))
            .unwrap();
    let row = to_installed(&declared[0], "id-1".to_string(), 42);
    assert_eq!(row.server_id, "id-1");
    assert_eq!(row.qualified_name, "fs");
    assert_eq!(row.display_name, "fs");
    assert_eq!(row.command_kind, CommandKind::Node);
    assert_eq!(row.installed_at, 42);
    assert!(row.env_keys.is_empty());
}

#[test]
fn merging_credentials_replaces_removes_and_keeps() {
    let mut stored = BTreeMap::new();
    stored.insert("A".to_string(), "old".to_string());
    stored.insert("B".to_string(), "keep".to_string());
    stored.insert("C".to_string(), "gone".to_string());
    let mut written = BTreeMap::new();
    written.insert("A".to_string(), "new".to_string());
    written.insert("C".to_string(), String::new());
    written.insert("D".to_string(), "added".to_string());

    let merged = merge_credentials(&stored, &written);
    assert_eq!(merged.get("A").unwrap(), "new");
    assert_eq!(merged.get("B").unwrap(), "keep");
    assert!(!merged.contains_key("C"));
    assert_eq!(merged.get("D").unwrap(), "added");
}
