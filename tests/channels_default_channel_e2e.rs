//! Default messaging channel: `channels_set_default` / `channels_get_default`.
//!
//! # Why this target exists
//!
//! Both methods are in the domain-e2e gate's uncovered list
//! (`scripts/check-domain-e2e-coverage.mjs`), which counts only
//! `tests/**/*_e2e.rs` targets. Before this file the complete picture was:
//!
//! * `app/src/services/api/channelConnectionsApi.ts:255`, `:263` — **shipped
//!   frontend code** calls both.
//! * `app/src/services/api/channelConnectionsApi.test.ts:47-60` — a Vitest that
//!   asserts the client *emits the right method name* against a mocked
//!   transport. It never reaches the core, so it would keep passing if both
//!   controllers were deleted tomorrow.
//! * Four WDIO/Playwright specs call `channels_set_default` as **setup** and
//!   assert nothing about it.
//!
//! So the round trip had no coverage anywhere above a mocked transport.
//!
//! # What is actually worth asserting
//!
//! A bare set→get round trip is the weak half. `channels/proactive.rs:100-116`
//! documents the interesting invariant: `channels_set_default` mutates the live
//! proactive subscriber's `active_channel` handle **in place** via
//! `set_runtime_active_channel`, "so a default-channel switch from the UI takes
//! effect without a restart" (issue #3712, "switch default channel
//! Telegram<->Discord"), and `proactive.rs:226-240` reads that handle back on
//! every proactive delivery. The failure that bug describes is: config
//! persists, the live handle does not update, proactive messages keep going to
//! the old channel until the process restarts — and a test that only reads the
//! config back cannot see it.
//!
//! **That assertion IS made here** — see
//! `set_default_applies_to_the_live_proactive_handle`. An earlier revision of
//! this file omitted it, on the reasoning that `proactive.rs:135-138` says
//! `set_runtime_active_channel` is "a no-op when no subscriber has registered a
//! handle (e.g. unit tests)", so the assertion would pass vacuously. **That was
//! wrong**: `register_active_channel_handle` (`proactive.rs:129`) is `pub`, so
//! the test registers its own handle and no channel runtime is needed. The
//! non-vacuity control is asserting the handle reads `None` *before* the call —
//! without it, "still unset afterwards" would look like a pass.
//!
//! So this target asserts everything reachable from the RPC surface — the round
//! trip across the wire, persistence, canonicalisation, the documented `"web"`
//! fallback — plus the live-apply half that is the actual #3712 regression.
//!
//! No network: `api_url` points at a closed port.
//!
//! Run with: `cargo test -p openhuman-cli --test channels_default_channel_e2e`

use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::Duration;

use axum::http::header::AUTHORIZATION;
use reqwest::StatusCode;
use serde_json::{json, Value};
use tempfile::{tempdir, TempDir};

use openhuman_core::core::auth::{init_rpc_token, CORE_TOKEN_ENV_VAR};
use openhuman_core::core::jsonrpc::build_core_http_router;

const TEST_RPC_TOKEN: &str = "channels-default-channel-e2e-token";

static AUTH_INIT: OnceLock<()> = OnceLock::new();
static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

// ── Env isolation ────────────────────────────────────────────────────

struct EnvVarGuard {
    key: &'static str,
    old: Option<String>,
}

impl EnvVarGuard {
    fn set_to_path(key: &'static str, path: &Path) -> Self {
        let old = std::env::var(key).ok();
        // SAFETY: every caller holds `env_lock()`, which serialises the
        // process-global env mutations this type performs.
        unsafe { std::env::set_var(key, path.as_os_str()) };
        Self { key, old }
    }

    fn set(key: &'static str, value: &str) -> Self {
        let old = std::env::var(key).ok();
        // SAFETY: see `set_to_path`.
        unsafe { std::env::set_var(key, value) };
        Self { key, old }
    }

    fn unset(key: &'static str) -> Self {
        let old = std::env::var(key).ok();
        // SAFETY: see `set_to_path`.
        unsafe { std::env::remove_var(key) };
        Self { key, old }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.old {
            // SAFETY: teardown runs inside the same `env_lock()` critical
            // section as setup.
            Some(value) => unsafe { std::env::set_var(self.key, value) },
            // SAFETY: see above.
            None => unsafe { std::env::remove_var(self.key) },
        }
    }
}

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    match ENV_LOCK.get_or_init(|| Mutex::new(())).lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn ensure_rpc_auth() {
    AUTH_INIT.get_or_init(|| {
        // SAFETY: runs once, behind a `OnceLock`, before any server starts.
        unsafe { std::env::set_var(CORE_TOKEN_ENV_VAR, TEST_RPC_TOKEN) };
        let token_dir = std::env::temp_dir().join("openhuman-channels-default-e2e-auth");
        init_rpc_token(&token_dir).expect("init rpc auth token");
    });
}

fn write_config(openhuman_dir: &Path) {
    std::fs::create_dir_all(openhuman_dir).expect("create .openhuman");
    let cfg = r#"api_url = "http://127.0.0.1:9"
default_model = "channels-default-e2e-model"
default_temperature = 0.2

[secrets]
encrypt = false

[local_ai]
enabled = false

[memory]
provider = "none"
embedding_provider = "none"
embedding_model = "none"
embedding_dimensions = 0
"#;
    std::fs::write(openhuman_dir.join("config.toml"), cfg).expect("write config.toml");
    let _: openhuman_core::config::Config =
        toml::from_str(cfg).expect("test config must match schema");
}

struct Harness {
    rpc_base: String,
    home: std::path::PathBuf,
    _tmp: TempDir,
    _guards: Vec<EnvVarGuard>,
    join: tokio::task::JoinHandle<Result<(), std::io::Error>>,
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.join.abort();
    }
}

impl Harness {
    /// Every `config.toml` under this harness's `$HOME`, as `(path, body)`.
    ///
    /// Deliberately a search rather than one hard-coded path. The core scopes
    /// config per user (`config/schema/load/dirs.rs` resolves a root
    /// `~/.openhuman` plus a `users/` tree), so `Config::save` does not
    /// necessarily write back to the seed file this harness planted at
    /// `~/.openhuman/config.toml`. Asserting one path would test where this
    /// test *guessed* the file lives rather than whether the choice persisted
    /// — an earlier revision did exactly that and failed against working code.
    fn configs_on_disk(&self) -> Vec<(std::path::PathBuf, String)> {
        fn walk(dir: &Path, out: &mut Vec<(std::path::PathBuf, String)>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, out);
                } else if path.file_name().is_some_and(|n| n == "config.toml") {
                    if let Ok(body) = std::fs::read_to_string(&path) {
                        out.push((path, body));
                    }
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.home, &mut out);
        out
    }
}

async fn setup() -> Harness {
    ensure_rpc_auth();

    let tmp = tempdir().expect("tempdir");
    let home = tmp.path().to_path_buf();
    write_config(&home.join(".openhuman"));

    let guards = vec![
        EnvVarGuard::set_to_path("HOME", &home),
        EnvVarGuard::unset("OPENHUMAN_WORKSPACE"),
        EnvVarGuard::unset("BACKEND_URL"),
        EnvVarGuard::unset("VITE_BACKEND_URL"),
        EnvVarGuard::unset("OPENHUMAN_API_URL"),
        EnvVarGuard::set("OPENHUMAN_KEYRING_BACKEND", "file"),
    ];

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind rpc listener");
    let addr = listener.local_addr().expect("rpc listener addr");
    let router = build_core_http_router(false);
    let join = tokio::spawn(async move { axum::serve(listener, router).await });

    Harness {
        rpc_base: format!("http://{addr}"),
        home,
        _tmp: tmp,
        _guards: guards,
        join,
    }
}

async fn rpc(base: &str, id: i64, method: &str, params: Value) -> Value {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("client");
    let url = format!("{}/rpc", base.trim_end_matches('/'));
    let response = client
        .post(&url)
        .header(AUTHORIZATION, format!("Bearer {TEST_RPC_TOKEN}"))
        .json(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }))
        .send()
        .await
        .unwrap_or_else(|err| panic!("POST {url} {method}: {err}"));
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "HTTP transport should accept {method}"
    );
    response
        .json::<Value>()
        .await
        .unwrap_or_else(|err| panic!("json for {method}: {err}"))
}

fn ok<'a>(value: &'a Value, context: &str) -> &'a Value {
    if let Some(error) = value.get("error") {
        panic!("{context}: unexpected JSON-RPC error: {error}");
    }
    value
        .get("result")
        .unwrap_or_else(|| panic!("{context}: missing result: {value}"))
}

fn payload<'a>(value: &'a Value, context: &str) -> &'a Value {
    let outer = ok(value, context);
    outer
        .get("data")
        .or_else(|| outer.get("result"))
        .unwrap_or(outer)
}

fn active_channel(value: &Value, context: &str) -> String {
    payload(value, context)
        .get("active_channel")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{context}: payload has no string `active_channel`: {value}"))
        .to_string()
}

async fn set_default(harness: &Harness, id: i64, channel: &str) -> Value {
    rpc(
        &harness.rpc_base,
        id,
        "openhuman.channels_set_default",
        json!({ "channel": channel }),
    )
    .await
}

async fn get_default(harness: &Harness, id: i64) -> Value {
    rpc(
        &harness.rpc_base,
        id,
        "openhuman.channels_get_default",
        json!({}),
    )
    .await
}

// ── Tests ────────────────────────────────────────────────────────────

/// The round trip the four existing specs use as setup and never assert.
///
/// `channels_get_default` is read back over the wire rather than out of the
/// config file on purpose: reading the file would prove persistence but not
/// that the getter serves it, and the getter is the half the UI calls.
#[tokio::test]
async fn set_default_then_get_default_returns_the_chosen_channel() {
    let _lock = env_lock();
    let harness = setup().await;

    let set = set_default(&harness, 1, "telegram").await;
    assert_eq!(
        active_channel(&set, "channels_set_default"),
        "telegram",
        "channels_set_default should echo back the channel it just set"
    );

    let got = get_default(&harness, 2).await;
    assert_eq!(
        active_channel(&got, "channels_get_default"),
        "telegram",
        "channels_get_default should return the channel channels_set_default just stored"
    );
}

/// The switch must survive as a *persisted* choice, not just an in-memory one.
///
/// `proactive.rs:114-115` says the choice "is also persisted to
/// `config.channels_config.active_channel`, which seeds the handle on next
/// start". If that write is lost, the default silently reverts on restart —
/// which a same-process round trip cannot see.
#[tokio::test]
async fn set_default_persists_the_choice_to_config_on_disk() {
    let _lock = env_lock();
    let harness = setup().await;

    let before = harness.configs_on_disk();
    assert!(
        !before.is_empty(),
        "precondition: the harness should have planted at least one config.toml under $HOME"
    );
    assert!(
        before
            .iter()
            .all(|(_, body)| !body.contains("active_channel")),
        "precondition: no config.toml should already name an active_channel, otherwise the \
         assertion below cannot tell a write from a pre-existing value — found: {:?}",
        before.iter().map(|(path, _)| path).collect::<Vec<_>>()
    );

    set_default(&harness, 1, "discord").await;

    let after = harness.configs_on_disk();
    let persisted: Vec<&std::path::PathBuf> = after
        .iter()
        .filter(|(_, body)| body.contains("active_channel"))
        .map(|(path, _)| path)
        .collect();
    assert!(
        !persisted.is_empty(),
        "channels_set_default returned ok but no config.toml under $HOME gained an \
         `active_channel` key, so the choice will not survive a restart (#3712). Searched: {:?}",
        after.iter().map(|(path, _)| path).collect::<Vec<_>>()
    );
    assert!(
        after
            .iter()
            .any(|(_, body)| body.contains("active_channel") && body.contains("discord")),
        "a config.toml gained an `active_channel` key but not the channel that was set — \
         files carrying the key: {persisted:?}"
    );
}

/// Canonicalisation: the handler lower-cases before storing
/// (`schemas.rs:332`), so a mixed-case switch from the UI must not produce a
/// default that no comparison downstream matches.
#[tokio::test]
async fn set_default_canonicalises_channel_case() {
    let _lock = env_lock();
    let harness = setup().await;

    let set = set_default(&harness, 1, "TeleGram").await;
    assert_eq!(
        active_channel(&set, "channels_set_default(TeleGram)"),
        "telegram",
        "channels_set_default should canonicalise the channel to lower case before storing it"
    );

    let got = get_default(&harness, 2).await;
    assert_eq!(
        active_channel(&got, "channels_get_default"),
        "telegram",
        "channels_get_default should return the canonicalised form, not the caller's casing"
    );
}

/// #3712 — the live-apply half, and the one that a config-only test cannot see.
///
/// `set_default_channel` (`ops/connect/status.rs:92-99`) does two things: it
/// persists `channels_config.active_channel`, and it calls
/// `set_runtime_active_channel` so proactive routing follows the switch without
/// a restart. The other tests in this file cover the first. If only the second
/// broke, every one of them would still pass while proactive messages kept
/// going to the old channel until the process restarted — which is exactly the
/// bug #3712 describes.
#[tokio::test]
async fn set_default_applies_to_the_live_proactive_handle() {
    let _lock = env_lock();
    let harness = setup().await;

    // The channel runtime is what registers this in production
    // (`start_channels.rs:463`). Registering it here is what makes the
    // assertion below meaningful rather than a no-op; `proactive.rs:129`
    // exposes it for exactly this reason and the latest registration wins.
    let live_channel: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(None));
    openhuman_core::channels::proactive::register_active_channel_handle(Arc::clone(&live_channel));

    // Non-vacuity control. Without it, a `set_runtime_active_channel` that did
    // nothing at all would leave the handle `None` and the assertion below
    // would be indistinguishable from "never ran".
    assert_eq!(
        live_channel.read().expect("read live handle").clone(),
        None,
        "precondition: the freshly registered handle must start empty, otherwise the \
         post-call assertion cannot tell a write from a pre-existing value"
    );

    set_default(&harness, 1, "discord").await;

    assert_eq!(
        live_channel.read().expect("read live handle").clone(),
        Some("discord".to_string()),
        "channels_set_default persisted the choice but did not update the live proactive \
         routing handle, so proactive messages keep going to the old channel until the \
         process restarts (#3712)"
    );
}

/// The documented fallback. `handle_get_default` (`schemas.rs:348`) ends in
/// `.unwrap_or_else(|| "web".to_string())`, so a fresh install answers `web`
/// rather than erroring or returning null — the UI renders this value directly.
#[tokio::test]
async fn get_default_falls_back_to_web_before_anything_is_set() {
    let _lock = env_lock();
    let harness = setup().await;

    let got = get_default(&harness, 1).await;
    assert_eq!(
        active_channel(&got, "channels_get_default(fresh)"),
        "web",
        "a fresh install should report `web` as the default messaging channel"
    );
}
