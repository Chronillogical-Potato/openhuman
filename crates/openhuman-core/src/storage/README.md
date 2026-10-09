# storage

The process's storage backend on the
[tinystoragedrivers](https://github.com/tinyhumansai/tinystoragedrivers)
ports, vendored through `vendor/tinyagents/vendor/tinystoragedrivers`.

One URL picks it: `OPENHUMAN_STORAGE_URL`, else `[storage] url` in
`config.toml` (`config::StorageConfig`). With neither set, the desktop
default, nothing is opened and every domain keeps the classic on-disk layout
under the workspace.

| URL | Driver | Cargo feature |
| --- | --- | --- |
| `memory` | in-process, keeps nothing | always |
| `sqlite:<path>` (a `.db` file, or a directory with one file per database) | SQLite | `storage-sqlite` |
| `mongodb://…/<db>`, `mongodb+srv://…/<db>` | MongoDB, one database shared by every scope | `storage-mongodb` |
| `file:<dir>` | JSON and JSONL files | `storage-file` |

A URL for a driver the build lacks fails at `open`, naming the feature. None
of the features are in the shipped desktop product yet; a cloud build turns
on `storage-mongodb`.

## Entry points

- `configured_url(&Config)` / `url_from(env, &Config)`: the URL in effect.
- `open(url)`: parse and open a backend (credentials are redacted in logs).
- `install(backend)` / `installed()` / `clear()`: the process slot domains
  read the backend from.
- `scope_for_agent(agent_id)`: the storage scope an agent's records live
  under, the same mapping TinyAgents' `DriverSessionStores` uses, so every
  domain agrees.
- `driver_is_shared(driver)` / `installed_is_shared()`: whether other
  processes may write the same backend (MongoDB). Boot-time recovery, such
  as the orphaned-run sweep, is skipped on a shared backend.
- `current_scope()` / `current_scoped()`: the acting agent's scope (`local`
  on a single-user host; an error in SaaS mode with no acting agent) and the
  installed backend under it.
- `block_on(future)`: runs a storage future from synchronous store code on
  one shared runtime thread.
- `documents::Repo` and `documents::compare_and_swap`: the base the domain
  stores build on. A `Repo` holds one domain's scoped document handle,
  declares its collections and runs each call; `compare_and_swap` is the
  guarded-`UPDATE` loop.

## Consumers

- The session store: `openhuman_rpc::session_store::install_for_host` opens
  the configured backend before boot and installs `DriverSessionStores`
  over it. See that module's README.
- Domain stores that switch to the document port when a backend is
  installed, each in a `store_documents.rs` beside its SQLite `store.rs`:
  approvals (`security::approval`), paired devices (`security::devices`),
  notifications (`desktop::notifications`) and task sources
  (`integrations::task_sources`).
- tinyflows' own stores on the ports (`tinyflows-drivers`), picked per call
  the same way: cron jobs and runs (`cron::store`, `CronDocuments`), the flow
  catalog and drafts (`flows::store`, `flows::draft_store`,
  `FlowCatalogDocuments`), per-flow engine state and dedup settlement
  (`flows::tinyflows::state::FlowState`), and the flow-run checkpointer
  (`DriverCheckpointer`). The delegation graph's checkpointer uses
  tinyagents-graph's `DriverCheckpointer`.
- `block_on_anyhow(future)`: `block_on` for those stores, whose errors are
  `anyhow::Error` (a typed `FlowUpdateError` passes through unchanged).
- Approvals (`security::approval`), through `store_documents.rs`.
- Secrets (`storage::secrets`): the keyring's user secrets
  (`security::keyring::get` / `set` / `delete`) and the credential stores'
  files (`auth-profiles.json`, `http-credentials.json`) become encrypted
  documents (`DocumentSecrets`, `enc2:`) in the acting agent's scope. Each
  scope's data key is derived with HKDF-SHA256 from the keyring master key
  (`OPENHUMAN_KEYRING_MASTER_KEY` / `_FILE`, else the OS keychain); with no
  master key they fail closed. The config encryption key stays on the
  process keyring, because `config.toml` is loaded before any agent acts.

## Background work and agent scopes

Work done inside an agent's turn runs under that agent's `CoreContext`
(`session_agent`, set for embed agents and SaaS user agents), so with a
backend installed its records land in that agent's scope. Background work
runs under the process default context and on its own would only see
`local`. `storage::agents` closes the gap:

- `registered(context)`: `CoreContext::derive_with` records every agent
  context it builds (live, by agent id), and the agent id is written to the
  backend's `local` scope (`storage_agents`) so a restarted process still
  knows it.
- `for_each_scope(label, step)`: runs `step` for `local`, then under each
  known agent's context — its live one, or the default context acting for it
  (`CoreContext::for_agent`). `for_each_agent` skips `local`.
- `within_agent(agent, fut)` / `context_for(agent)`: re-enter an agent's
  scope when background work learned whose record it is handling.

Users: the cron scheduler (`cron::scheduler::tick_agents`), the task-source
poller, the flows boot sweep and schedule-trigger reconcile, the run reaper,
and the device tunnel (a paired device's frames run as the agent that paired
it, `security::devices::owner`). Without a backend these run once, as
before. In SaaS mode agent ids are not recorded and `local` is skipped;
per-user background work there is `user_agents::background`.

## Boundaries

The ports, drivers, scopes and conformance suites are tinystoragedrivers';
the session store over them is `tinyagents-session`. This domain only
resolves the URL, opens the backend, and holds it for the process. The
`[storage]` section is bootstrap configuration and is never read from
storage itself.
