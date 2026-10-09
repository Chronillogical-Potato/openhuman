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

## Consumers

- The session store: `openhuman_rpc::session_store::install_for_host` opens
  the configured backend before boot and installs `DriverSessionStores`
  over it. See that module's README.

## Boundaries

The ports, drivers, scopes and conformance suites are tinystoragedrivers';
the session store over them is `tinyagents-session`. This domain only
resolves the URL, opens the backend, and holds it for the process. The
`[storage]` section is bootstrap configuration and is never read from
storage itself.
