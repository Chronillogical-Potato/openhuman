# openhuman-cli

This crate builds the `openhuman-core` binary, a handful of developer and
benchmark binaries, and every root `tests/*.rs` and `examples/*.rs` target in
the repository. The core (`crates/openhuman-core`, package `openhuman`) is a
library with no backend client and no JSON-RPC server, and it declares no
bin, test or example targets of its own. Anything that must run as a process
against the hosted backend, or test the core the way a host boots it, lives
here.

## How it works

### Where it sits

```text
                         openhuman-cli
                     (bins, tests, examples)
                      |        |         |
                      v        v         v
        openhuman-tinyhumans  openhuman-rpc   openhuman-core
          (SDK transport,     (JSON-RPC       (domains, CLI
           hosted proxies,     server,         dispatcher,
           session owner)      run_server*)    controller registry)
                |                  |
                v                  |
         openhuman-embed           |
                |                  |
                +--------+---------+
                         v
                  openhuman-core
```

The crate depends on the core directly (every bin and test names
`openhuman_core::` paths), on `openhuman-tinyhumans` for the backend
transport, and on `openhuman-rpc` (with its `server` feature) for the
JSON-RPC server that `run` and `serve` start.

### Startup of `openhuman-core`

`src/main.rs` is short and runs in a fixed order:

1. `restore_default_sigpipe()` resets `SIGPIPE` to the default on unix, so
   piping output into `head` ends the process quietly instead of panicking
   on `EPIPE`.
2. `dotenvy::dotenv()` loads a repo-local `.env` before Sentry starts, so a
   DSN defined only there is visible. The CLI dispatcher later runs
   `load_dotenv_for_cli`, which honors `OPENHUMAN_DOTENV_PATH`.
3. With the `crash-reporting` feature, `sentry::init` resolves the DSN from
   `OPENHUMAN_CORE_SENTRY_DSN`, then `OPENHUMAN_SENTRY_DSN`, at runtime and
   then at compile time. Its `before_send` drops known-noise event classes
   through the `openhuman_core::core::observability::is_*_event` predicates
   (transient provider failures, budget and credit exhaustion, session
   expiry, connectivity blips, stale releases, and others), strips
   `server_name`, attaches only the account id as the Sentry user, and
   scrubs secrets from messages and exception values with
   `openhuman_core::core::log_redaction::scrub_secrets`. The release tag is
   `openhuman@<version>[+<sha>]`, matching the frontend.
4. `openhuman_tinyhumans::install(InstallOptions::default())` installs the
   SDK-backed backend transport, registers the hosted RPC proxies and the
   Jev ranker. A failure here exits with status 1.
5. `openhuman_rpc::server::install_cli_server()` hands the core the JSON-RPC
   server launcher, which the core cannot depend on.
6. `openhuman_core::run_core_from_args(&args)` loads `.env` again with the
   CLI rules, applies any startup restart delay, initializes the keyring
   master key and dispatches the subcommand.

### Subcommands

The dispatcher is `openhuman_core::core::cli::run_from_cli_args`. Global
options go before the command: `-m`/`--model` (alias `--model-id`) and
`-p`/`--provider` (alias `--provider-id`) set process-local overrides that are
never written back to the config file.

| Command | What it does |
| --- | --- |
| `run`, `serve` | Start the HTTP JSON-RPC and Socket.IO server. Flags: `--host`, `--port`, `--jsonrpc-only` (no Socket.IO), `--headless-api` (JSON-RPC only, no background services), `-v`/`--verbose`. |
| `call --method <name> [--params '<json>' \| --params-stdin]` | Invoke one controller in-process (no server needed) and print the JSON result. Runs on a runtime with the agent worker stack size, so methods that run a turn work. |
| `mcp`, `mcp-server` | Run the stdio MCP server. No banner, since stdout carries JSON-RPC. |
| `agent <dump-prompt \| dump-all \| prompt-size \| list>` | Inspect agent definitions and rendered prompts (`core/agent_cli.rs`), for example `agent dump-prompt --agent <id> --json --with-tools`. |
| `sentry-test [--message <text>] [--panic]` | Send a test event to verify Sentry wiring. Reports a disabled-build error without `crash-reporting`. |
| `tui`, `chat` | Print a migration notice: the terminal UI is the separate `openhuman-tui` executable. |
| `<namespace> <function> [--param value ...]` | Generic dispatch to any registered controller, for example `skills ...` or `voice ...`. `<namespace> --help` lists functions; `<namespace> <function> --help` lists parameters. |

With no arguments or `--help`, it prints usage and the registered namespaces.
The banner goes to stderr so stdout stays clean JSON.

### Tests and examples are targets of this crate

Cargo only auto-discovers `tests/` and `examples/` beside the manifest, and
the files live at the repository root. So the manifest turns discovery off
(`autotests = false`, `autoexamples = false`, `autobins = false`,
`autobenches = false`) and declares each target explicitly with a path back
to the root:

```toml
[[test]]
name = "json_rpc_e2e"
path = "../../tests/json_rpc_e2e.rs"
required-features = ["voice"]
```

Every new `tests/<name>.rs` or `examples/<name>.rs` file needs such an entry.
`pnpm rust:layout` (`scripts/ci/check-openhuman-rust-layout.mjs`) fails on a
missing or stale entry, and on any target table in the core manifest.

Two targets aggregate whole directories, so files there need no entry. The
shared root `build.rs` (wired with `build = "../../build.rs"`) globs them into
generated module lists:

- `raw_coverage_all` (`tests/raw_coverage_all.rs`) includes every
  `tests/raw_coverage/*.rs` suite as a module. It needs `voice` and
  `inference`.
- `in_process_all` (`tests/in_process_all.rs`) includes every
  `tests/in_process/*.rs` suite. These boot the core router in the test
  process and are gate-free.

Folding the suites into one binary each links the large `openhuman` rlib once
instead of once per file. Suites that need a process of their own (global
`OnceCell`s, a real keyring, spawning the binary) stay separate targets.

A target that names symbols behind a product gate carries
`required-features`, so a contributor `cargo test` skips it instead of failing
to compile. The CI product lanes pass `--features
"$(scripts/ci/product-features.sh)"`, which turns them back on. Current gated
targets: `observability_smoke` (`crash-reporting`), `computer_bali_live_e2e`
(`modules`), `x402_twit_sh_live` (`web3`), `json_rpc_e2e` (`voice`),
`raw_coverage_all` (`voice`, `inference`) and `media_generation_e2e`
(`media`).

In-process suites that reach the (mock) backend call
`tinyhumans_boot::boot()` from `tests/support/tinyhumans_boot.rs` first, which
runs `openhuman_tinyhumans::install`. Without it every backend call answers
`BACKEND_UNAVAILABLE:`. Suites that spawn the `openhuman-core` binary get the
transport from `main.rs`.

## Layout

| Path | What it does |
| --- | --- |
| `Cargo.toml` | All `[[bin]]`, `[[test]]` and `[[example]]` targets, feature forwarding. |
| `src/main.rs` | The `openhuman-core` binary entry point described above. |
| [`src/bin/`](src/bin/README.md) | Developer and benchmark binaries: `test-mcp-stub`, `openhuman-fleet`, `rss-bench`, `library-profile`, `tool-search-bench`, `tool-dialect-bench`. |
| `../../tests/*.rs` | 27 `[[test]]` targets, including the two aggregators. See [`tests/README.md`](../../tests/README.md). |
| `../../examples/*.rs` | 2 `[[example]]` targets: `embed_headless` and `embed_kernel`. |
| `../../build.rs` | Shared build script: generates the `raw_coverage_all` and `in_process_all` module lists and exports `OPENHUMAN_REPOSITORY_ROOT`. |

## Targets

| Target | Source | Required features |
| --- | --- | --- |
| `openhuman-core` | `src/main.rs` | none |
| `test-mcp-stub` | `src/bin/test_mcp_stub.rs` | none |
| `openhuman-fleet` | `src/bin/fleet.rs` | `http-server`, `bin-tools` |
| `tool-search-bench` | `src/bin/tool_search_bench.rs` | none (`jev` for the Jev ranker) |
| `tool-dialect-bench` | `src/bin/tool_dialect_bench.rs` | none |
| `rss-bench` | `src/bin/rss_bench.rs` | `rss-bench` |
| `library-profile` | `src/bin/library_profile/main.rs` | `rss-bench` (add `rss-bench-dhat` for heap profiles) |

## Features

The default set mirrors the core's contributor default plus
`openhuman-tinyhumans/default` and `jev`. Every core gate (`http-server`,
`inference`, `voice`, `web3`, `channels`, `media`, `modules`, and the rest) is
forwarded to both `openhuman-core` and `openhuman-tinyhumans`, so the product
lanes' feature list resolves here unchanged; `scripts/ci/check-feature-forwarding.mjs`
checks the chain. `e2e-test-support` and `rss-bench` forward to the core only.
Gates local to this crate:

| Feature | Purpose |
| --- | --- |
| `jev` | The Jev `tool_search` ranker (`openhuman-tinyhumans/jev`) plus the clients `tool-search-bench` measures it with. |
| `crash-reporting` | Sentry init in `main.rs` and the `observability_smoke` target. |
| `bin-tools` | `clap` for `openhuman-fleet`. |
| `rss-bench-dhat` | dhat heap profiling for `library-profile`; implies `rss-bench`. |

## Boundaries

- Subcommand parsing and dispatch live in the core
  (`crates/openhuman-core/src/core/cli.rs`, `core/agent_cli.rs`). New
  functionality is a controller registered in `core/all.rs`, reached through
  the generic namespace dispatch, not a new branch in `cli.rs`.
- The JSON-RPC server, Socket.IO and the listener belong to
  `crates/openhuman-rpc`. The backend transport and login belong to
  `crates/openhuman-tinyhumans`.
- The terminal UI is `crates/openhuman-tui`; the desktop host is
  `crates/openhuman-app`, which embeds the core directly and does not use
  this binary.

## Gotchas

- Sentry's `before_send` filters are defense in depth. The primary
  suppression for each noise class lives at its emit site in the core; add
  new filters there first.
- The `[[test]]` comment block in `Cargo.toml` about product-gated targets
  still says autodiscovery stays on for the other targets. It does not:
  `autotests = false`, and every target is declared.
- Do not export `CARGO_TARGET_DIR`; the repository already configures a
  shared target directory.

## Tests

```bash
cargo build -p openhuman-cli --bin openhuman-core
cargo test  -p openhuman-cli --test in_process_all
cargo test  -p openhuman-cli --test json_rpc_e2e --features "$(bash scripts/ci/product-features.sh)"
pnpm test:rust          # scripts/test-rust-with-mock.sh, the canonical runner
pnpm debug rust <filter>
```

`main_tests.rs` (built with `crash-reporting`) covers the release tag,
environment resolution and secret scrubbing in `main.rs`.
