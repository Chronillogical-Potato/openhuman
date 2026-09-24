//! E2E coverage for the `tree_summarizer` ingestion buffer.
//!
//! Gate controller `tree_summarizer_ingest` had zero references under `tests/`
//! or `app/test/` before this target.
//!
//! The claim under test is a **boundary contract**, which is why this is an RI
//! suite rather than a unit test. `memory/tree/tree_runtime/ops.rs` defaults the
//! timestamp host-side, deliberately, and says why:
//!
//!     // Defaulted here rather than driver-side, exactly as before: the reply
//!     // echoes the instant the content was filed under, and a timestamp the
//!     // driver resolved would disagree with the one reported here by however
//!     // long the call took to cross.
//!
//! If that defaulting ever migrates across the bus, the reply starts reporting
//! a different instant than the one the content was actually filed under. The
//! field stays present and stays plausible, so every downstream ordering or
//! dedupe decision keyed on that echo drifts silently. Nothing catches it
//! today.
//!
//! Run with: `cargo test -p openhuman-cli --test tree_summarizer_e2e`

#[path = "support/memory_module.rs"]
mod memory_module;

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use axum::http::header::AUTHORIZATION;
use chrono::{DateTime, SecondsFormat, TimeZone, Utc};
use serde_json::{json, Value};
use tempfile::TempDir;

use openhuman_core::core::auth::{init_rpc_token, CORE_TOKEN_ENV_VAR};
use openhuman_core::core::jsonrpc::build_core_http_router;

const TEST_RPC_TOKEN: &str = "tree-summarizer-e2e-token";
const NAMESPACE: &str = "tree-summarizer-e2e";

static AUTH_INIT: OnceLock<()> = OnceLock::new();
static MEMORY_SEAMS_INIT: OnceLock<()> = OnceLock::new();
static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static TEST_HOME: OnceLock<TempDir> = OnceLock::new();

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    match ENV_LOCK.get_or_init(|| Mutex::new(())).lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn test_home() -> &'static Path {
    TEST_HOME
        .get_or_init(|| tempfile::tempdir().expect("tree summarizer tempdir"))
        .path()
}

struct EnvVarGuard {
    key: &'static str,
    old: Option<String>,
}

impl EnvVarGuard {
    fn set_to_path(key: &'static str, path: &Path) -> Self {
        let old = std::env::var(key).ok();
        // SAFETY: every caller holds env_lock() for the whole of setup and
        // teardown, so these process-global mutations are serialised.
        unsafe { std::env::set_var(key, path.as_os_str()) };
        Self { key, old }
    }

    fn unset(key: &'static str) -> Self {
        let old = std::env::var(key).ok();
        // SAFETY: see set_to_path.
        unsafe { std::env::remove_var(key) };
        Self { key, old }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.old {
            // SAFETY: see EnvVarGuard::set_to_path.
            Some(value) => unsafe { std::env::set_var(self.key, value) },
            // SAFETY: see EnvVarGuard::set_to_path.
            None => unsafe { std::env::remove_var(self.key) },
        }
    }
}

/// Publish the module host policy for this process.
///
/// `tree_summarizer_ingest` reaches the loaded tinymemory module through
/// `tree_guard`, and without this every call fails with "the module host policy
/// was never published ... call modules::memory::set_modules_policy during
/// boot". Installed on its own 8 MB thread for the same reason the sibling
/// suites do: the policy build recurses deeper than a default test stack.
///
/// The `#[cfg(feature = "modules")]` body is evaluated against **openhuman-cli**'s
/// features, not the core's. `openhuman-cli`'s own `modules` flag is OFF in its
/// default set even though `openhuman-core/modules` is on transitively, so a
/// bare `cargo test -p openhuman-cli --test tree_summarizer_e2e` compiles this
/// to a no-op and every test here fails on the message above. Run it the way
/// `scripts/test-rust-e2e.sh` does:
///
///     RUST_MIN_STACK=67108864 cargo test -p openhuman-cli \
///       --features "$(bash scripts/ci/product-features.sh)" \
///       --test tree_summarizer_e2e
fn ensure_memory_seams() {
    MEMORY_SEAMS_INIT.get_or_init(|| {
        std::thread::Builder::new()
            .name("tree-summarizer-e2e-seams".to_string())
            .stack_size(8 * 1024 * 1024)
            .spawn(|| {
                let config = std::sync::Arc::new(openhuman_core::config::Config::default());
                #[cfg(feature = "modules")]
                openhuman_core::modules::memory::set_modules_policy(config);
            })
            .expect("spawn tree summarizer seam installer")
            .join()
            .expect("tree summarizer seam installer panicked");
    });
}

fn ensure_rpc_auth() {
    AUTH_INIT.get_or_init(|| {
        // SAFETY: called once, under env_lock() via the test body.
        unsafe { std::env::set_var(CORE_TOKEN_ENV_VAR, TEST_RPC_TOKEN) };
        let token_dir = std::env::temp_dir().join("openhuman-tree-summarizer-e2e-auth");
        std::fs::create_dir_all(&token_dir).expect("mkdir token dir");
        init_rpc_token(&token_dir).expect("init rpc token");
    });
}

fn write_config(dir: &Path) {
    std::fs::create_dir_all(dir).expect("mkdir openhuman home");
    let cfg = r#"
default_model = "e2e-mock-model"
default_temperature = 0.7

[secrets]
encrypt = false

[memory_tree]
embedding_strict = false
"#;
    std::fs::write(dir.join("config.toml"), cfg).expect("write config");
    let user_dir = dir.join("users").join("local");
    std::fs::create_dir_all(&user_dir).expect("mkdir user dir");
    std::fs::write(user_dir.join("config.toml"), cfg).expect("write user config");
}

async fn serve() -> (String, tokio::task::JoinHandle<Result<(), std::io::Error>>) {
    ensure_memory_seams();
    ensure_rpc_auth();
    memory_module::settle().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let handle =
        tokio::spawn(async move { axum::serve(listener, build_core_http_router(false)).await });
    (format!("http://{addr}"), handle)
}

async fn rpc(base: &str, id: i64, method: &str, params: Value) -> Value {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("client");
    let url = format!("{}/rpc", base.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .header(AUTHORIZATION, format!("Bearer {TEST_RPC_TOKEN}"))
        .json(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
        .send()
        .await
        .unwrap_or_else(|e| panic!("POST {url}: {e}"));
    assert!(
        resp.status().is_success(),
        "HTTP error {} for {method}",
        resp.status()
    );
    resp.json::<Value>()
        .await
        .unwrap_or_else(|e| panic!("json parse for {method}: {e}"))
}

fn ok(v: &Value, ctx: &str) -> Value {
    if let Some(err) = v.get("error") {
        panic!("{ctx}: JSON-RPC error: {err}");
    }
    let outer = v
        .get("result")
        .unwrap_or_else(|| panic!("{ctx}: missing result: {v}"));
    // RpcOutcome wraps its payload under an inner "result" alongside "logs".
    outer
        .get("result")
        .cloned()
        .unwrap_or_else(|| outer.clone())
}

fn setup() -> (EnvVarGuard, EnvVarGuard, EnvVarGuard, EnvVarGuard, PathBuf) {
    let home = test_home();
    let openhuman_home = home.join(".openhuman");
    let home_guard = EnvVarGuard::set_to_path("HOME", home);
    let ws = EnvVarGuard::unset("OPENHUMAN_WORKSPACE");
    let backend = EnvVarGuard::unset("BACKEND_URL");
    let vite = EnvVarGuard::unset("VITE_BACKEND_URL");
    write_config(&openhuman_home);
    (home_guard, ws, backend, vite, openhuman_home)
}

// ── Tests ────────────────────────────────────────────────────────────

/// An explicit `timestamp` is echoed back exactly, not replaced by `now()`.
///
/// The fixture instant is fixed and far in the past, so a handler that resolved
/// its own `now()` could not satisfy this by coincidence.
#[tokio::test]
async fn ingest_echoes_the_caller_supplied_timestamp() {
    let _lock = env_lock();
    let (_home, _ws, _backend, _vite, _oh) = setup();

    let (rpc_base, _join) = serve().await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 2021-03-04T05:06:07Z — fixed, and years away from any plausible `now()`.
    let fixed: DateTime<Utc> = Utc
        .with_ymd_and_hms(2021, 3, 4, 5, 6, 7)
        .single()
        .expect("fixed fixture instant");
    let fixed_rfc3339 = fixed.to_rfc3339_opts(SecondsFormat::Secs, true);

    let before = Utc::now();
    let response = rpc(
        &rpc_base,
        401,
        "openhuman.tree_summarizer_ingest",
        json!({
            "namespace": NAMESPACE,
            "content": "Engineering note filed under a caller-supplied instant.",
            "timestamp": fixed_rfc3339,
        }),
    )
    .await;
    let result = ok(&response, "tree_summarizer_ingest with explicit timestamp");

    // Fixture guard: if the write did not happen there is nothing to assert
    // about, and `buffered` is the handler's own statement that it did.
    assert_eq!(
        result.get("buffered"),
        Some(&json!(true)),
        "ingest did not report buffered=true — got {result}"
    );

    let echoed = result
        .get("timestamp")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("no `timestamp` in {result}"));
    let echoed: DateTime<Utc> = DateTime::parse_from_rfc3339(echoed)
        .unwrap_or_else(|e| panic!("unparseable timestamp {echoed:?}: {e}"))
        .with_timezone(&Utc);

    assert_eq!(
        echoed, fixed,
        "ingest must echo the instant the caller filed the content under ({fixed}), not one it \
         resolved itself. A driver-side default would disagree with the reply by however long the \
         call took to cross (memory/tree/tree_runtime/ops.rs)."
    );
    // Stated separately so a regression that returns `now()` names itself,
    // rather than only showing two timestamps that happen to differ.
    assert!(
        echoed < before,
        "echoed timestamp {echoed} is not the fixture instant but a value at or after this \
         test started ({before}) — the handler resolved its own clock"
    );

    assert_eq!(
        result.get("namespace"),
        Some(&json!(NAMESPACE)),
        "ingest must echo the namespace it filed under — got {result}"
    );
    assert_eq!(
        result.get("has_metadata"),
        Some(&json!(false)),
        "no metadata was sent, so has_metadata must be false — got {result}"
    );
    let path = result
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("no `path` in {result}"));
    assert!(
        !path.is_empty(),
        "ingest must report the buffer path the driver wrote"
    );
}

/// With no `timestamp`, the handler resolves one itself and it lands inside the
/// window this test brackets.
///
/// Bracketing both sides is what makes this non-vacuous: a handler returning a
/// constant, a zero instant or an unparseable string fails.
#[tokio::test]
async fn ingest_without_a_timestamp_files_under_the_host_clock() {
    let _lock = env_lock();
    let (_home, _ws, _backend, _vite, _oh) = setup();

    let (rpc_base, _join) = serve().await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // One second of slack each side: the assertion is "the host clock", not a
    // claim about RPC latency.
    let before = Utc::now() - chrono::Duration::seconds(1);
    let response = rpc(
        &rpc_base,
        402,
        "openhuman.tree_summarizer_ingest",
        json!({
            "namespace": NAMESPACE,
            "content": "Engineering note filed without an explicit instant.",
            "metadata": { "source": "tree-summarizer-e2e" },
        }),
    )
    .await;
    let after = Utc::now() + chrono::Duration::seconds(1);
    let result = ok(&response, "tree_summarizer_ingest without timestamp");

    assert_eq!(
        result.get("buffered"),
        Some(&json!(true)),
        "ingest did not report buffered=true — got {result}"
    );

    let echoed = result
        .get("timestamp")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("no `timestamp` in {result}"));
    let echoed: DateTime<Utc> = DateTime::parse_from_rfc3339(echoed)
        .unwrap_or_else(|e| panic!("unparseable timestamp {echoed:?}: {e}"))
        .with_timezone(&Utc);

    assert!(
        echoed >= before && echoed <= after,
        "an omitted timestamp must be defaulted to the host clock; {echoed} is outside the \
         window [{before}, {after}] this call was made in"
    );

    // The metadata this call *did* send must be reflected, so the two tests
    // together pin both branches of `has_metadata`.
    assert_eq!(
        result.get("has_metadata"),
        Some(&json!(true)),
        "metadata was sent, so has_metadata must be true — got {result}"
    );
}
