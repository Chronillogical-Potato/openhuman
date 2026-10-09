# session_store

The on-disk session store the desktop app, the CLI and the TUI install.
`SqliteSessionStores` implements TinyAgents' `SessionStoreProvider` port over
the layout OpenHuman has always written under its workspace: JSONL
transcripts, the SQLite run ledger, turn snapshots, and the key-value and
journal stores for run status, goals and todos. The core itself carries no
storage layout; it asks whichever provider the host installed. Compiled with
the crate's `session-store` feature, which `server` turns on and the TUI
enables by itself.

## How it works

```text
 host startup
   session_store::install()
     SqliteSessionStores::resolving(context_workspace_dir)
     -> agent::session_store::install(provider)
   CoreBuilder::build()
     provider.recover()
       turn_state::store::mark_all_interrupted    (turn snapshots)
       run_ledger::interrupt_orphaned_agent_runs  (run ledger)

 during a turn
   agent::session_store::current()
     provider.for_agent(agent id)
       stores_at(<current workspace>)
         transcripts   FileTranscriptLocator
         turn_states   TurnStateStore
         kv, journal   open_session_stores
```

`install()` puts a provider into the core's process slot
(`openhuman_core::agent::session_store::install`). It has to run before the
core boots, because boot calls the provider's `recover()`: turns an unclean
shutdown left in flight are marked interrupted, and run-ledger rows a dead
process left running are settled. Both `run_server*` shims in
[`../server/`](../server/README.md) call it, and the TUI calls it from
`crates/openhuman-tui/src/runner.rs`.

`install()` builds the provider with `resolving(context_workspace_dir)`,
so the workspace is looked up on every `for_agent` call rather than fixed at
construction. The desktop rebinds the workspace when a different user signs
in, and the next turn then reads and writes that user's files.
`context_workspace_dir` takes the workspace from the current `CoreContext`
and falls back to the default config's workspace (with a warning) when no
context is booted.

Every `for_agent(agent_id)` call returns stores over the same workspace,
whatever the agent id. This is the single-operator layout: all agents share
one workspace, as they always have. A multi-user host that needs per-agent
isolation supplies its own provider through
`openhuman_embed::RuntimeBuilder::session_store` instead.

The layout under `{workspace}`:

```text
session_raw/*.jsonl                          transcripts
session_db/sessions.db                       run ledger (SQLite)
memory/conversations/turn_states/...         turn snapshots
tinyagents_store/{kv,journal}/               run status, goals, todos,
                                             turn journal
```

## A storage-backed store instead

`install_for_host()` is what the server shims and the TUI call. With no
storage URL configured (`OPENHUMAN_STORAGE_URL`, else `[storage] url` in
`config.toml`) it is `install()` above, unchanged. With one, it opens that
backend (`openhuman_core::storage::open`), makes it the process's storage
backend, and installs TinyAgents' `DriverSessionStores` over it instead:

```text
 install_for_host()
   storage URL?   no  -> install()   (the layout above)
                  yes -> storage::open(url) -> storage::install(backend)
                         DriverSessionStores::new(backend)
                           .recover_on_open(driver != "mongodb")
                         -> agent::session_store::install(provider)
```

Every agent then gets its own storage scope (its id, or `sha256:` of an id
that is not a valid scope), so agents sharing one MongoDB database never see
each other's transcripts, turn states, records or journals. A single-process
backend (SQLite, memory, files) interrupts an agent's in-flight turns the
first time it is opened; MongoDB does not, because another process may own
them. A URL that cannot be parsed or opened fails the boot rather than
falling back to local files. `install_for_url` is the same with the URL
already resolved, for tests and hosts that read it themselves.

## Layout

| File | What it does |
| --- | --- |
| `mod.rs` | `SqliteSessionStores` (`at`, `resolving`), its `SessionStoreProvider` impl (`for_agent`, `recover`, `destination_key`, `workspace_dir`), `install()`, and `install_for_host()` / `install_for_url()`, which install `DriverSessionStores` when a storage URL is configured. |

## Key types and entry points

- `SqliteSessionStores::at(dir)`: stores under a fixed directory. Useful in
  tests and for a host whose workspace never changes.
- `SqliteSessionStores::resolving(f)`: stores under whatever directory `f`
  returns at call time.
- `install()`: installs a resolving provider over the current context's
  workspace as the process's session store.
- `destination_key()` and `workspace_dir()` both report the current
  workspace path; the core logs the destination key when a provider is
  installed.

## Boundaries

- The port (`SessionStoreProvider`, `AgentStores`), the transcript format
  and locator, turn states, the run ledger and the kv/journal stores are all
  `tinyagents-session` (`vendor/tinyagents`, repo `tinyhumansai/tinyagents`).
  This module only arranges those building blocks into OpenHuman's
  directory layout. Changes to the formats belong upstream.
- The process slot, the scoped override and `current()` belong to the core
  (`openhuman_core::agent::session_store`). The core and `openhuman-embed`
  reach session state only through that port, with a fallback to the same
  workspace files for hosts that install no store.
- Session identity (`SessionRef`: thread id plus agent id, no timestamp) and
  generation sealing on compaction are TinyAgents behavior, not this
  module's.

## Gotchas

- Install before `CoreBuilder::build`, or the boot-time recovery sweep runs
  against no provider and interrupted turns stay marked as running.
- This provider does not isolate agents. Do not use it for a multi-tenant
  host.

## Tests

`mod_tests.rs` runs the TinyAgents conformance suite against this layout,
checks that the workspace follows the resolver, and checks that recovery
interrupts turns left in flight. The `SqliteSessionStores` doc example also
runs as a doctest.

```bash
cargo test -p openhuman-rpc session_store
```
