# openhuman-tinyhumans

OpenHuman on the hosted TinyHumans backend.

```
openhuman-core   ──► knows the backend only through `api::transport::BackendTransport`
      ▲
openhuman-embed  ──► library facade (Runtime → Agent)
      ▲
openhuman-tinyhumans ──► implements the port with the vendored tinyhumans-sdk
```

The core has **no** `tinyhumans-sdk` dependency. Built and run on its own it
has no TinyHumans connection: agents, memory, skills, tools and RPC work, and
every hosted-backend surface (billing, `/agent-integrations/*` tools, channel
relay, cloud voice) answers with a typed `BACKEND_UNAVAILABLE:` error that
observability demotes. This crate is what turns that into a connected
runtime.

## Use

Library hosts:

```rust
use openhuman_tinyhumans::{embed::Workspace, RuntimeBuilder};

let runtime = RuntimeBuilder::new()
    .workspace(Workspace::Ephemeral)
    .api_key("th_...")
    .build()
    .await?;
```

`RuntimeBuilder` mirrors `openhuman_embed::RuntimeBuilder` method for method
and, on `build()`, installs the SDK transport and binds it to the runtime.

Hosts that boot the core themselves (desktop shell, TUI, CLI, test fixtures)
call `install` once before the first backend-touching dispatch:

```rust
openhuman_tinyhumans::install(openhuman_tinyhumans::InstallOptions::default())?;
```

## What lives here

| Module | Owns |
| --- | --- |
| `transport` | `SdkBackendTransport`: one `reqwest::Client` per `TransportProfile`, built from the core's `api::headers` so TLS, timeouts and `x-core-version` / `x-tauri-version` / `x-sdk-name` are exactly what the core specifies; SDK route policy; `tinyhumans_sdk::Error` → `BackendTransportError` |
| `install` | process-global installation, idempotent |
| `RuntimeBuilder` | embed builder + transport |
| `jwt` | the SDK's JWT readers, for hosts that already depend on this crate |

Routes and error classification stay in the core (`api/rest.rs`,
`integrations/client/`); this crate never adds a route the core does not
already name.

## Tests

```bash
cargo test -p openhuman-tinyhumans
```

`transport_tests.rs` pins the wire shape the core's classifiers depend on
(bearer vs `x-api-key`, attribution headers, `Status` / `Envelope` mapping,
SDK route refusal) against a wiremock backend.
