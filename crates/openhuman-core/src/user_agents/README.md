# user_agents

In SaaS mode (`core::runtime::Mode::Saas`), each user is served as one agent.
This domain maps a gateway user to that agent, lays out the agent's private
state, forces the config it runs with, and keeps the open agents of the
process. A single-user core never serves it: its controllers belong to
`DomainGroup::Operator`, which only `DomainSet::saas()` enables.

## Files

| File | Purpose |
| --- | --- |
| `types.rs` | `UserAgentId` (`u-` + 32 hex chars of `sha256(user_id)`), its metadata, and the operator-plane result types |
| `layout.rs` | `<root>/agents/<id>/{agent.toml, config.toml, workspace/, sandbox/}`, archived agents, and `agent_config`: the forced paths, memory binding and autonomy policy |
| `host.rs` | `AgentHost`: provisioning, lazy open, LRU and idle eviction (never of an agent in use), each agent's derived `CoreContext`, `current()` |
| `ops.rs` | `provision` / `deprovision` / `list` / `status`, returning `Outcome<T>` |
| `schemas.rs` | The `user_agents.*` controllers |

## Rules

- **Gateway user ids never leave `ops.rs`.** They are hashed into an agent id
  immediately and are never logged, stored or returned.
- **An agent's config is forced, not configured.**
  - Every path sits under the agent's directory.
  - Memory is bound to the agent (`[memory] agent_id`, `root = user:<id>`).
    That binding wins over definition pins and team roots.
  - The autonomy policy is on and supervised, with no auto-approval, no tool
    installation and no trusted roots.
- **The isolation boundary is the agent's `CoreContext`.** It carries the forced
  config and `session_agent = <id>`, and has no domain family or tool group of
  its own yet. Work for a user runs under it, which is what the config loader,
  the session store and the per-thread caches key on.
- **Deprovisioning archives.** The agent's directory moves to
  `<root>/deprovisioned/<id>-<unix-secs>/`. Nothing is deleted.
