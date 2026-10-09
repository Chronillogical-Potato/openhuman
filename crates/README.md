# crates

The Rust side of OpenHuman is seven crates. One library holds the business
logic; the others are layers above it that add a public facade, the JSON-RPC
protocol, the hosted TinyHumans connection, and the three executables people
run (the CLI binary, the terminal UI and the desktop app). Six of them are
members of the root Cargo workspace; `openhuman-app` is excluded and builds
with `--manifest-path crates/openhuman-app/Cargo.toml`.

## The crates

| Crate | Package / lib | What it is |
| --- | --- | --- |
| [`openhuman-core`](openhuman-core/README.md) | package `openhuman`, lib `openhuman_core` | The core library: every business domain under `src/<domain>/`, the controller contract and registry, in-process dispatch, the event bus and the CLI dispatcher. No JSON-RPC server, no backend client, no binary targets. |
| [`openhuman-embed`](openhuman-embed/README.md) | `openhuman-embed` | The typed library facade for running the core in-process in another product (`Runtime` then `Agent`). |
| [`openhuman-rpc`](openhuman-rpc/README.md) | `openhuman-rpc` | JSON-RPC 2.0 over the core: envelopes, the HTTP client (`http-client`), the server with Socket.IO and the `run_server*` entry points (`server`), and the on-disk session store (`session-store`). |
| [`openhuman-tinyhumans`](openhuman-tinyhumans/README.md) | `openhuman-tinyhumans` | The hosted-backend layer: the SDK-backed `BackendTransport`, `install()`, a `RuntimeBuilder` that boots connected, the hosted RPC proxies, the login and session owner, and the Jev ranker. The only crate allowed to depend on `tinyhumans-sdk`. |
| [`openhuman-cli`](openhuman-cli/README.md) | `openhuman-cli` | The `openhuman-core` binary, the developer and benchmark binaries, and every root `tests/*.rs` and `examples/*.rs` target. |
| [`openhuman-tui`](openhuman-tui/README.md) | `openhuman-tui` | The standalone terminal client, embedding the core in-process. |
| [`openhuman-app`](openhuman-app/README.md) | `openhuman-app` (lib `openhuman`) | The thin Tauri v2 desktop host. Runs the core and its JSON-RPC server as a tokio task. Outside the root workspace. |

## How they layer

Arrows point from a crate to what it depends on (normal `[dependencies]`,
taken from each `Cargo.toml`).

```text
  openhuman-app      openhuman-tui      openhuman-cli      (executables)
        |                  |                  |
        +------------------+------------------+
        |   each of the three depends on all three crates below
        |
        +-----------------------+-------------------------+
        |                       |                         |
        v                       v                         |
  openhuman-tinyhumans    openhuman-rpc                   |
    |          |          (server, client,                |
    |          |           session store)                 |
    |          v                |                         |
    |   vendor/tinyhumans-sdk   |                         |
    v                           |                         |
  openhuman-embed               |                         |
    |                           |                         |
    v                           v                         v
  +--------------------------------------------------------------+
  |                        openhuman-core                        |
  |    (domains, controller registry, BackendTransport port)     |
  +--------------------------------------------------------------+
```

The same edges as a list:

| Crate | Depends on (first-party) |
| --- | --- |
| `openhuman-core` | none |
| `openhuman-embed` | `openhuman-core` |
| `openhuman-rpc` | `openhuman-core` |
| `openhuman-tinyhumans` | `openhuman-embed`, `openhuman-core` (plus `vendor/tinyhumans-sdk`) |
| `openhuman-cli` | `openhuman-core`, `openhuman-tinyhumans`, `openhuman-rpc` (`server`) |
| `openhuman-tui` | `openhuman-core`, `openhuman-rpc` (`session-store`), `openhuman-tinyhumans` |
| `openhuman-app` | `openhuman-core`, `openhuman-rpc` (`http-client`, `server`), `openhuman-tinyhumans` (`jev`) |

Every executable depends on the core directly as well as through the layers,
because each names `openhuman_core::` paths. `openhuman-tinyhumans` also
depends on the core directly for surfaces embed does not re-export
(`backend::transport`, `core::all`).

## Why it is split this way

The core runs agents, memory, tools and controllers without any hosted
backend. It reaches the backend only through the `BackendTransport` port and
knows nothing of JSON-RPC, so it can be embedded with neither. The layers
above add those pieces, and each host installs what it needs at startup:

```text
 host main()
   openhuman_tinyhumans::install(..)         backend transport, hosted
                                             proxies, Jev ranker
   openhuman_rpc::server::install_cli_server()   (CLI: run/serve)
   boot the core  (run_core_from_args, CoreBuilder, embedded server)
```

A core with no transport installed answers backend calls with
`BACKEND_UNAVAILABLE:`. `cargo tree -p openhuman -i tinyhumans-sdk` must stay
empty, and the core must not depend on `openhuman-rpc`.

## Features

Cargo default features define the contributor build;
`scripts/ci/product-features.txt` defines the shipped product. A core gate is
forwarded along the library chain (`openhuman-embed`, then
`openhuman-tinyhumans`, then `openhuman-cli`), and the desktop app, which
builds with `default-features = false`, forwards product gates explicitly.
`scripts/ci/check-feature-forwarding.mjs` checks both.

## Build and test

```bash
cargo check --manifest-path Cargo.toml                       # workspace crates
cargo build -p openhuman-cli --bin openhuman-core
cargo check --manifest-path crates/openhuman-app/Cargo.toml  # desktop host
cargo test  -p openhuman-tinyhumans
pnpm test:rust
```

Root `tests/*.rs` and `examples/*.rs` are targets of `openhuman-cli`; see its
README for the explicit `[[test]]` entries they need.
