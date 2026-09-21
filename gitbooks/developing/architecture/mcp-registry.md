---
description: >-
  The host half of the dynamic, user-facing side of MCP-client support:
  browse servers on Smithery and the official MCP registry, declare the
  user's servers in one mcp.json document, connect them (persistence and
  supervision live in the vendored `tinymcp` crate), and surface their tools
  to agents via the unified tool registry.
icon: plug
---

# MCP Registry (`crates/openhuman-core/src/mcp/registry/`)

`crates/openhuman-core/src/mcp/registry/` is the **host half** of the dynamic, user-facing side of OpenHuman's Model Context Protocol client support: browsing the supported upstream registries (Smithery and the official modelcontextprotocol registry), reconciling the install store against the user's `mcp.json` document, persisting that, and (for servers launched as local subprocesses or HTTP-remote endpoints) supervising the connection lifecycle. Installed servers' tools are surfaced to agents via the unified tool registry (`crate::tools::registry`).

The registries are **browse-only**. There is no install-from-catalog path and no setup agent: a user who finds a server in the Registry tab opens its own page, reads the install instructions there, and declares the server in the mcp.json tab (`{ "mcpServers": { name: { command, args, env } | { url, headers } } }`). `config_doc.rs` is that document's contract and `config_ops.rs` the reconciliation (`mcp_clients_config_get` / `mcp_clients_config_set`).

The **client half** — both transports, the Smithery/official catalogs, the SQLite store, the live connection map, the subprocess supervisor, browser sign-in, and the write-audit log — moved to the vendored `tinymcp` crate (`vendor/tinymcp`). What lives in this directory is only what belongs to this application:

- `host.rs` (one level up, `crates/openhuman-core/src/mcp/host.rs`): the one `tinymcp` service this process holds per workspace, and config-to-`tinymcp` conversion.
- `registry/`: the `mcp_clients` RPC surface, the agent-facing tools, and the prompt-injection scan applied to remote tool definitions.
- `audit/` (sibling of `registry/`): the RPC surface over `tinymcp`'s write-audit log.
- `server/` (sibling of `registry/`): the `openhuman-core mcp` stdio/HTTP server that exposes this application's own tools to external MCP hosts — see [MCP Server](../mcp-server.md). This is the *server* side and did not move.

> **Naming note**: the Rust module path is `crate::mcp::registry` (`crates/openhuman-core/src/mcp/registry/`), but the RPC namespace and on-disk SQLite filename stay `mcp_clients` for backward compatibility with existing frontend code and stored user state. Grep both names when chasing call sites.

All payload types (`InstalledServer`, `McpTool`, `ConnStatus`, the Smithery/official-registry DTOs) come from `tinymcp_bus` and are re-exported under `registry::types`, not redefined here. Types for the *static, config-declared* server set (`[[mcp_client.servers]]` in `config.toml`) and the shared HTTP/stdio transport primitives are re-exported from `crate::mcp::config_servers` and `crate::mcp::http_client` (thin `pub use tinymcp::...` modules in `mcp/mod.rs`, not directories).

```text
                 ┌────────────────────────────────────────────────┐
   Registries ───►         tinymcp (catalogs, store, supervisor)   │
                 └────────────────────┬───────────────────────────┘
                                      │ browse / declare
                                      ▼
                          ┌──────────────────────┐
   Frontend (Skills UI) ─►│  ops.rs / schemas.rs │  RPC controllers (mcp_clients_*)
                          └──────────┬───────────┘
                                     │ delegates to
                                     ▼
                          ┌──────────────────────┐
                          │   host::for_config    │  the tinymcp service this
                          │   / host::try_service │  workspace's host holds
                          └──────────┬───────────┘
                                     │ tools_safe_for_agent (prompt-injection scan)
                                     ▼
                          tool_registry (agents)

                          supervisor_events.rs ── reconnect-supervisor ticks → DomainEvent
```

## Server transport model

An `InstalledServer` (from `tinymcp_bus`, re-exported as `registry::types::InstalledServer`) carries a `Transport` discriminator with stdio and HTTP-remote variants — a local subprocess (`npx`, `uvx`, or a direct binary) speaking stdio JSON-RPC, or a hosted server (the majority of what Smithery lists) dialled over streamable HTTP. A declaration in `mcp.json` becomes such a row directly (`config_doc::to_installed`): `command`/`args` → stdio, `url` → HTTP-remote, `env`/`headers` → the credential table. `tinymcp` dials whatever the row says; this domain only decides what the row says.

## Boot-time spawn and the reconnect supervisor

Installed servers are connected as the core comes up, and `mcp::registry::supervisor` (see `crates/openhuman-core/src/mcp/registry/mod.rs`) runs `tinymcp::Supervisor::tick` on an interval against every open workspace host, turning each tick's report into `supervisor_events::publish` calls. Errors never block boot; a broken MCP install should not gate the desktop app starting. Non-nominal ticks (stays-down, restored, parked) become `DomainEvent`s that reach the Event Log and the desktop notification bridge (#5931).

## Layout

| Path                        | Role                                                                                                                                                                                    |
| ---------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `mod.rs`                    | Module docs, `types`/`connections` re-export shims over `tinymcp_bus`/`super::host`, the reconnect-supervisor loop, the OAuth-callback completion helper, and `tools_safe_for_agent` (the prompt-injection scan applied to every remote tool definition). |
| `host.rs` (parent `mcp/`)   | One `tinymcp` service per workspace, opened on first use; proxy resolution for MCP traffic (`proxy_for_mcp`).                                                                          |
| `helpers.rs`                | Shared RPC-handler plumbing: `RpcOutcome` encoding, identifier guards, workspace-service resolution, credential-name injection.                                                        |
| `ops.rs`                    | `mcp_clients_*` RPC handler implementations (uninstall, list, browse, connect/disconnect, tool call, `update_env`, registry settings). One-to-one with `schemas.rs` handlers; publishes `DomainEvent`s `tinymcp` does not. |
| `config_doc.rs`             | The `mcp.json` contract: `render` (store → document, credential names only), `parse` (document → declarations, refusing what the store cannot carry), `same_dial`, `to_installed`, `merge_credentials`. |
| `config_ops.rs`             | `mcp_clients_config_get` / `config_set`: replace the store with what the document declares — add, rewrite in place under the same `server_id`, or uninstall — merging write-only credentials and connecting new enabled servers in the background. |
| `schemas/` (`mod.rs`, `registry.rs`, `handlers.rs`, `params.rs`) | Controller schemas + handler dispatch. Re-exported from `mod.rs` as `all_mcp_registry_controller_schemas` / `all_mcp_registry_registered_controllers`.                    |
| `bus.rs`                    | `DomainEvent` subscriber (`McpClientEventSubscriber`) that logs `McpServer*` / `McpClientToolExecuted` lifecycle events.                                                               |
| `supervisor_events.rs`      | Turns a `tinymcp::Supervisor` tick report into this domain's `DomainEvent`s, stamped with the workspace whose host was ticked.                                                          |
| `tools.rs`                  | Agent-facing `mcp_registry_*` tools (search catalog, inspect/list/connect/disconnect/call) — thin shims over `ops.rs`. The one mutator (`mcp_registry_uninstall`) ships default-OFF behind the `mcp_manage` toggle; there is no install tool. Distinct from the generic `mcp_list_servers`/`mcp_call_tool` bridge tools. |
| `stub.rs`                   | The disabled facade compiled when the `mcp` feature is off.                                                                                                                            |

## Public surface

The exports from `mod.rs` are intentionally narrow:

```rust
pub use schemas::{
    all_controller_schemas as all_mcp_registry_controller_schemas,
    all_registered_controllers as all_mcp_registry_registered_controllers,
    schemas as mcp_registry_schemas,
};

pub use types::{ConnStatus, InstalledServer, McpTool};
```

`types` and `connections` are thin `pub mod`s re-exporting the `tinymcp_bus` wire vocabulary and `super::host`-backed lookups, respectively — this domain does not define its own copies. Everything else (`bus`, `ops`, `setup_ops`, `supervisor_events`, `tools`) is `pub mod` for in-crate callers but not re-exported from `mod.rs`.

## Calls into

- `crate::mcp::host`: resolves the `tinymcp` service for a workspace (`host::for_config`, `host::try_service`, `host::all_hosts`).
- `tinymcp` / `tinymcp_bus`: the catalogs, store, connection map, supervisor, OAuth flow, and wire types.
- `crate::security::prompt_injection::scan_tool_definition`: the scan `tools_safe_for_agent` applies before a remote tool reaches the model.
- `crate::tools::registry`: installed servers' tools land here so agents see them alongside native tools.
- `crate::core::bus::BUS`: publishes `DomainEvent`s (`McpToolRejected`, lifecycle events, supervisor observations).

## Called by

- Core startup, via `mcp::host::init` and the reconnect-supervisor loop.
- Frontend Connections UI: the **MCP Servers** page (`McpServersTab`) is three tabs over the `openhuman.mcp_clients_*` RPC namespace — **Servers** (rows, status, the credential form: connect/disconnect, `update_env`, sign-in), **mcp.json** (`config_get` / `config_set`, the only way a server is added or removed) and **Registry** (`registry_search`, browse-only; a row opens the server's own page in the browser). `registry_settings_get` / `registry_settings_set` hold the Smithery / official-registry credentials (secret values are write-only). Agents use connected servers through `mcp_agent` (`use_mcp_server`); they do not install them.

## Tests

Focused `*_tests.rs` siblings cover each file: `bus_tests.rs`, `ops_tests.rs`, `schemas_tests.rs`, `setup_ops_tests.rs`, `supervisor_events_tests.rs`, `tools_tests.rs`.

## Related

- [`mcp/registry/mod.rs`](https://github.com/tinyhumansai/openhuman/blob/main/crates/openhuman-core/src/mcp/registry/mod.rs) and [`mcp/mod.rs`](https://github.com/tinyhumansai/openhuman/blob/main/crates/openhuman-core/src/mcp/mod.rs): the authoritative rustdoc this page mirrors.
- `crate::mcp::config_servers` / `crate::mcp::http_client` (`mcp/mod.rs`): re-export modules for the static config-declared server set and the shared transport primitives, both implemented in `tinymcp`.
- `crates/openhuman-core/src/mcp/audit/`: the RPC surface over `tinymcp`'s write-audit log.
- [MCP Server](../mcp-server.md): the `openhuman-core mcp` stdio/HTTP server side, which did not move to `tinymcp`.
- [Agent Harness](agent-harness.md): how the agent ends up calling MCP tools through `tool_registry`.
- [Architecture overview](../architecture.md): where this fits in the wider system.
