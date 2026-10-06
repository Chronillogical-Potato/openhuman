//! Host policy for the live session-store dual-write and the shadow read.
//!
//! The work itself (`write_live_turn`, `shadow_read_compare`) lives in
//! `tinyagents_session::transcript::import::live`; what stays here is the
//! OpenHuman decision of *whether* it runs: the `AgentConfig` flags
//! (`session_dual_write`, `session_shadow_reads`, both default ON), the
//! `OPENHUMAN_SESSION_DUAL_WRITE` / `OPENHUMAN_SESSION_SHADOW_READS` kill
//! switches (a falsey value forces the feature off, an env var can never force
//! it on; this mirrors the `OPENHUMAN_APPROVAL_GATE` idiom), and the
//! `RunContext.stores` registration of the session KV store.

use std::sync::Arc;

use tinyagents_harness::store::Store;
use tinyagents_session::transcript::import::ops::{open_session_stores, SessionStores};

/// Kill-switch env var for the live session-store dual-write. The config flag
/// (`AgentConfig::session_dual_write`) defaults ON; setting this env var to a
/// falsey value forces the mirror OFF regardless of config. See
/// [`dual_write_enabled`].
const DUAL_WRITE_ENV: &str = "OPENHUMAN_SESSION_DUAL_WRITE";

/// Kill-switch env var for the store-backed session shadow read. The config
/// flag (`AgentConfig::session_shadow_reads`) defaults ON since the Phase 2
/// parity soak; setting this env var to a falsey value forces the shadow read
/// OFF even when the flag is ON. It can never force the shadow read ON. See
/// [`shadow_reads_enabled`].
const SHADOW_READ_ENV: &str = "OPENHUMAN_SESSION_SHADOW_READS";

/// Whether `var` is set to a case-insensitive falsey value
/// (`0`/`false`/`no`/`off`/`disable`/`disabled`). Unset — or any non-falsey
/// value — is not a kill. Read live (not cached) so a config reload / env
/// change is honored on the next turn/read.
fn env_kill_switch_engaged(var: &str) -> bool {
    match std::env::var(var) {
        Ok(v) => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off" | "disable" | "disabled"
        ),
        Err(_) => false,
    }
}

/// Whether the `OPENHUMAN_SESSION_DUAL_WRITE` kill switch is engaged (set to a
/// falsey value). Unset — or any non-falsey value — leaves the mirror driven by
/// the config flag. Read live (not cached) so a config reload / env change is
/// honored on the next turn.
fn kill_switch_engaged() -> bool {
    env_kill_switch_engaged(DUAL_WRITE_ENV)
}

/// Store-registry name under which the session KV store is registered on each
/// turn's `RunContext.stores` (issue #4249, 04.1). Slash-free so it round-trips
/// the crate `FileStore` name sanitizer. This is a forward-looking,
/// harness-visible handle to the same `tinyagents_store` KV tree the live
/// dual-write mirrors into; readers stay legacy until 04.2.
pub const TINYAGENTS_SESSION_KV_STORE: &str = "openhuman_sessions";

/// Whether the live session-store dual-write is enabled for this turn.
///
/// `config_enabled` is the `AgentConfig::session_dual_write` flag, which
/// **defaults ON**. The `OPENHUMAN_SESSION_DUAL_WRITE` env var is a pure kill
/// switch: an explicit falsey value (case-insensitive
/// `0`/`false`/`no`/`off`/`disable`/`disabled`) forces the mirror OFF regardless
/// of config; otherwise the config flag wins. Read live (never cached) so a
/// config reload / env change is honored on the next turn. This keeps a clean
/// 04.2 seam (reads can flip independently) while making the mirror the default
/// so new turns land in the store without opt-in.
///
/// Always off under a host session store that is not file-backed
/// ([`crate::agent::session_store::replaces_files`]): it is already the only
/// record, and there are no transcript files to mirror.
pub fn dual_write_enabled(config_enabled: bool) -> bool {
    let killed = kill_switch_engaged();
    let replaced = crate::agent::session_store::replaces_files();
    let enabled = config_enabled && !killed && !replaced;
    log::debug!(
        "[session-store] dual-write decision config_enabled={config_enabled} kill_switch={killed} host_store={replaced} enabled={enabled}"
    );
    enabled
}

/// Open the session KV store as an `Arc<dyn Store>` for registration on the
/// per-turn `RunContext.stores` under [`TINYAGENTS_SESSION_KV_STORE`], honoring
/// the dual-write flag (config default ON + env kill switch).
///
/// Best-effort: `None` when the dual-write is disabled **or** the config (hence
/// workspace) cannot be resolved. When present it is the exact same
/// `{workspace}/tinyagents_store/kv` `FileStore` the importer and the live
/// dual-write use, so a harness-side reader (04.2+) sees identical records. The
/// journal (`JsonlAppendStore`, an `AppendStore` rather than a `Store`) is not
/// registrable on the `StoreRegistry`; the dual-write opens it directly.
///
/// With a host session store that is not file-backed it is the current
/// agent's key-value store, whatever the dual-write flag says: there are no
/// files to mirror.
pub async fn session_kv_store() -> Option<Arc<dyn Store>> {
    if crate::agent::session_store::replaces_files() {
        if let Some(stores) = crate::agent::session_store::current() {
            log::debug!(
                "[session-store] registering the host session store's kv on RunContext.stores"
            );
            return Some(stores.kv);
        }
    }
    let cfg = match crate::config::Config::load_or_init().await {
        Ok(cfg) => cfg,
        Err(err) => {
            log::warn!("[session-store] cannot resolve config for store registration: {err:#}");
            return None;
        }
    };
    if !dual_write_enabled(cfg.agent.session_dual_write) {
        log::debug!(
            "[session-store] dual-write disabled; skipping RunContext session-store registration"
        );
        return None;
    }
    let workspace = cfg.workspace_dir;
    let SessionStores { kv, .. } = open_session_stores(&workspace);
    log::debug!(
        "[session-store] opened session kv store for RunContext.stores workspace={}",
        workspace.display()
    );
    Some(Arc::new(kv))
}

/// Whether the store-backed session **shadow read** is enabled for this read.
///
/// `config_enabled` is the `AgentConfig::session_shadow_reads` flag, which
/// **defaults ON** since the Phase 2 parity soak, as `session_dual_write`
/// already did. The
/// `OPENHUMAN_SESSION_SHADOW_READS` env var is a pure kill switch: an explicit
/// falsey value (case-insensitive `0`/`false`/`no`/`off`/`disable`/`disabled`)
/// forces the shadow read OFF regardless of config; it can never force it ON.
/// Read live (never cached) so a config reload / env change is honored on the
/// next read. Mirrors the [`dual_write_enabled`] flag/env idiom exactly: the
/// env var can only ever force OFF, never ON.
pub fn shadow_reads_enabled(config_enabled: bool) -> bool {
    let killed = env_kill_switch_engaged(SHADOW_READ_ENV);
    let enabled = config_enabled && !killed;
    log::debug!(
        "[session_shadow_read] decision config_enabled={config_enabled} kill_switch={killed} enabled={enabled}"
    );
    enabled
}
