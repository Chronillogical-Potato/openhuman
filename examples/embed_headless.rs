//! Embed the OpenHuman core as a library — no HTTP, no background services.
//!
//! Demonstrates the library API, `openhuman_embed::Runtime`: build a
//! fully-initialized core with [`ServiceSet::none`] (no ports bound, no cron/channels/login-gated services) AND
//! [`DomainSet::harness`] (only the agent + memory + threads + config + security
//! domain families are live — the gate families flows/skills/mcp/meet/channels/
//! web3/voice/media and the catch-all `platform` are off, so their controllers
//! are unknown-method, their agent tools absent, and their stores/subscribers
//! never initialize). Dispatch RPC methods in-process through
//! [`Runtime::core_runtime`]'s `invoke` — the exact same path the HTTP `/rpc`
//! handler and the CLI use. The workspace is ephemeral (the library default),
//! so running this touches nothing in `~/.openhuman`. Turns belong to agents:
//! see `Runtime::agent` for the typed path.
//!
//! Run with:
//!
//! ```bash
//! cargo run --example embed_headless
//! ```
//!
//! To instead expose the core over HTTP, use `openhuman-rpc`'s host entries
//! (`openhuman_rpc::host::serve_desktop` / the `openhuman-core serve` CLI),
//! which serve a runtime like this one over JSON-RPC. Widen the runtime surface
//! by swapping `DomainSet::harness()` for `DomainSet::full()`.

use openhuman_embed::{DomainSet, HostKind, Runtime, ServiceSet};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Library embedders own logging; a simple env_logger keeps the example
    // self-contained. (`RUST_LOG=info cargo run --example embed_headless`)
    let _ = env_logger::builder().is_test(false).try_init();

    // Initialize the core on an ephemeral workspace. `HostKind::Cli` selects
    // the standalone (non-desktop) bootstrap path; `DomainSet::harness()` builds
    // the embeddable agent core; `ServiceSet::none()` means no transport and no
    // background services are started.
    let runtime = Runtime::builder()
        .host_kind(HostKind::Cli)
        .domains(DomainSet::harness())
        .services(ServiceSet::none())
        .build()
        .await?;
    let core = runtime.core_runtime();

    // Dispatch a couple of RPC methods in-process — no network involved.
    // `core.version` and `openhuman.ping` (a legacy alias for the built-in
    // `core.ping`) are always available regardless of the DomainSet — they are
    // transport built-ins, not domain controllers — so they succeed even under
    // `harness()`.
    let version = core
        .invoke("core.version", serde_json::json!({}))
        .await
        .map_err(|e| anyhow::anyhow!("core.version failed: {e}"))?;
    println!("core.version -> {version}");

    let ping = core
        .invoke("openhuman.ping", serde_json::json!({}))
        .await
        .map_err(|e| anyhow::anyhow!("openhuman.ping failed: {e}"))?;
    println!("openhuman.ping -> {ping}");

    Ok(())
}
