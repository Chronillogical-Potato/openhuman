---
description: >-
  Beyond the curated OAuth connectors - browse thousands of MCP servers and a
  90,000-entry Skills catalog, and let OpenHuman act as an MCP server itself.
icon: blocks
---

# MCP Servers & Skills

The [one-click OAuth integrations](README.md) are the curated path. Beyond them, OpenHuman opens up the wider open-tooling ecosystem in two ways: the **Model Context Protocol (MCP)** registry and the **Skills** catalog. OpenHuman can also expose _itself_ as an MCP server to other clients.

---

## MCP servers (thousands)

OpenHuman has a built-in **MCP registry** that browses the open MCP ecosystem and lets you install servers locally as new typed tools for the agent.

- **Two upstream registries, merged.** Discovery fans out in parallel to [Smithery.ai](https://smithery.ai) and the official [`registry.modelcontextprotocol.io`](https://registry.modelcontextprotocol.io), then merges the results: thousands of servers across both.
- **Search → install → connect.** Search the catalog, view a server's details, and install it. A local install spawns the server as a stdio subprocess; deployed servers connect over HTTP.
- **Supervised connections.** Installed servers are persisted in a local SQLite store (`mcp_clients/mcp_clients.db`) with their command, args, and transport. A supervisor loop keeps enabled servers connected, probing every ~60s with per-server exponential backoff. Each non-nominal probe outcome (a slow answer, a dropped transport, a reconnect that succeeded or failed) shows up in **Settings → Developer → Event Log** under the `mcp` badge. An answered probe does not, so a healthy server stays silent. A server that stays down, comes back after failing, or cannot start because its launcher runtime is missing raises a notification.

Once connected, an MCP server's tools are available to the agent exactly like native tools.

> The catalogs are open-ended and grow on their own. The "5,000+" figure reflects the combined Smithery + official ecosystem size, not a fixed list baked into OpenHuman.

### OpenHuman as an MCP server

OpenHuman can run the other way around, too. `openhuman-core mcp` exposes OpenHuman over stdio as an MCP server, offering memory tools (recall with citations, fetch, list, learn and forget) and optional web search to clients like Claude Desktop. See [MCP Server](../../developing/mcp-server.md) for setup.

---

## Skills (90,000-entry catalog)

**Skills** are a large, browsable catalog of agent skills (`SKILL.md`-style capability bundles) aggregated from multiple upstream sources (HermesHub, ClawHub, LobeHub, and more).

- **One aggregated catalog.** Sourced from HermesHub (configurable via `OPENHUMAN_SKILL_REGISTRY_CATALOG_URL`), the catalog runs to roughly **90,000 entries**. Each entry carries id, name, description, source, author, version, tags, platforms, a download URL, and license.
- **Paged and cached.** The catalog is served by the `tinyskills` skill registry: the app asks for one page at a time (25 rows, filtered and ranked by the core), and opening an entry loads its full detail. The catalog is warmed on boot in the background (without blocking startup) and stored at `~/.openhuman/skill-registry/hermes.json` (`OPENHUMAN_SKILL_REGISTRY_CACHE_DIR` moves it). While a refresh runs, or when the registry cannot be reached, the tab keeps showing the saved catalog and says so, with a Retry.
- **Guarded downloads.** Installs fetch `SKILL.md` over HTTPS only, refuse non-public addresses and re-check every redirect. Entries without a `SKILL.md` (LobeHub agents, portal pages) show "Not installable" with a link to their source page. `OPENHUMAN_SKILL_REGISTRY_DOWNLOAD_BASE_URL` serves every `SKILL.md` from a mirror; `OPENHUMAN_SKILL_INSTALL_ALLOW_LOCAL_HTTP=1` allows a loopback `http` mirror for local fixtures.
- **Metadata-first.** OpenHuman's in-app skills runtime (the old QuickJS sandbox) has been **removed**. Skills are now a metadata catalog you browse and install from the **Connections → Skills** tab, not code executing inside the app. Availability varies per entry: some expose a direct `SKILL.md` download, others point to external hosting.

---

## See also

- [Third-party Integrations](README.md): the curated managed-auth OAuth connectors.
- [MCP Server](../../developing/mcp-server.md): running OpenHuman as an MCP server.
- [Available Tools](../native-tools/README.md): the native tools that ship by default.
