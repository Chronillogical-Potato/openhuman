//! Knowledge-graph relation round trip over the real JSON-RPC router.
//!
//! # Why this target exists
//!
//! `tests/memory_graph_sync_e2e.rs` was deleted in `cc99ba9c67` ("build(memory):
//! cut the engine out of the test build", 2026-09-09) along with seventeen other
//! test files, because it drove `tinymemory_core` directly and the engine left
//! `[dev-dependencies]` in that commit. The deletion was structural — nothing
//! decided the behaviour had stopped mattering — and nothing replaced it.
//! Matrix row 11.1.1 records the hole as "the former graph-sync E2E was removed".
//!
//! # What was and was not left behind
//!
//! Two things DO cover part of this ground, and this file deliberately does not
//! duplicate them:
//!
//! * `memory::ops::kv_graph_tests::graph_handlers_roundtrip_relation_rows` is a
//!   unit test that upserts a relation and queries it back through a real
//!   module-backed provider. It calls the `ops` functions **directly**, so it
//!   proves the handler wiring but never touches JSON-RPC dispatch or the
//!   `serde` layer that builds `GraphUpsertParams` / `GraphQueryParams` from a
//!   wire payload.
//! * `tests/worker_c_modules_e2e.rs` names `openhuman.memory_graph_upsert` and
//!   `openhuman.memory_graph_query` in a 68-method loop that calls each with
//!   `json!({})` and asserts `assert_rpc_completed`. That helper tolerates an
//!   error response as long as it is not `unknown method:`, which is the
//!   strongest thing a probe with no arguments can honestly claim. It is a
//!   registration check and cannot observe a graph that writes and never reads
//!   back.
//!
//! So what has no coverage at all, and is what this file asserts:
//!
//! 1. The **wire path** — that a relation posted as JSON to `/rpc` survives
//!    deserialisation, dispatch, the guard, the module and serialisation back
//!    out. Both params structs take `namespace` as `#[serde(default)] Option`,
//!    so a wire payload takes a different path through serde than a
//!    hand-built struct literal does.
//! 2. **Namespace isolation** — that a relation stored under one namespace is
//!    not returned when querying another.
//!
//! On (2): the deleted test asserted `all_rows.len() >= graph_rows.len()`, i.e.
//! that an unfiltered query is a superset of a namespace-scoped one. That
//! assertion is **weak on its own** — a driver that ignored the namespace filter
//! entirely would return everything for both queries and still satisfy it. This
//! file keeps the superset check because it pins the documented relationship,
//! and adds the isolation check that actually fails when the filter breaks.
//!
//! Deliberately NOT covered here: `memory_tree_graph_export`. Despite the name
//! it exports the summary-tree / contacts graph
//! (`memory::read_rpc::graph::graph_export_rpc`), not the relation triples
//! `graph_upsert` writes. It needs its own case.
//!
//! No network: the config points `api_url` at a closed port and disables
//! embeddings, and every assertion is local.
//!
//! # Running it
//!
//! ```text
//! cargo test -p openhuman-cli --features modules --test memory_graph_roundtrip_e2e
//! ```
//!
//! **`--features modules` is required.** `openhuman-cli`'s `modules` feature is
//! NOT in its `default` set (`crates/openhuman-cli/Cargo.toml:415`, `:423`), and
//! without it `modules::memory::set_modules_policy` is `#[cfg]`-compiled away —
//! so `ensure_memory_seams` becomes a silent no-op and every graph call answers
//! "the module host policy was never published". The memory driver is
//! module-backed, so there is no non-module path to assert against. The same
//! applies to the sibling `tests/memory_roundtrip_e2e.rs`, which does not say
//! so and fails 6/6 locally without the flag; that is environmental, not a
//! defect in either file. CI's `rust-e2e` lane supplies both the feature and a
//! built `TINYMEMORY_TEST_MODULE`.
//!
//! A large stack is also needed — `RUST_MIN_STACK=67108864` — because
//! publishing the policy touches deeply nested config types.

use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use axum::http::header::AUTHORIZATION;
use reqwest::StatusCode;
use serde_json::{json, Value};
use tempfile::{tempdir, TempDir};

use openhuman_core::core::auth::{init_rpc_token, CORE_TOKEN_ENV_VAR};
use openhuman_core::core::jsonrpc::build_core_http_router;

const TEST_RPC_TOKEN: &str = "memory-graph-roundtrip-e2e-token";

static AUTH_INIT: OnceLock<()> = OnceLock::new();
static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static MEMORY_SEAMS_INIT: OnceLock<()> = OnceLock::new();
/// One tempdir shared by every test in this target.
///
/// `set_modules_policy` publishes a process-global policy exactly once, so the
/// workspace it names must be stable for the whole binary. A per-test tempdir
/// would leave tests 2..n writing through a policy that points at test 1's
/// directory. Test isolation comes from the unique namespace each test
/// generates instead.
static TEST_ROOT: OnceLock<TempDir> = OnceLock::new();

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

fn test_root() -> &'static TempDir {
    TEST_ROOT.get_or_init(|| tempdir().expect("memory graph roundtrip tempdir"))
}

/// Publish the module host policy the memory module is loaded through.
///
/// Without this every graph call answers "the module host policy was never
/// published, so module 'tinymemory' cannot be loaded" — the memory driver is
/// module-backed, and the HTTP router alone does not boot it. Mirrors
/// `tests/memory_roundtrip_e2e.rs::ensure_memory_seams`, including the larger
/// stack: publishing the policy touches deeply nested config types that
/// overflow the default 2 MiB test-thread stack.
fn ensure_memory_seams(workspace: &Path) {
    MEMORY_SEAMS_INIT.get_or_init(|| {
        let workspace = workspace.to_path_buf();
        std::thread::Builder::new()
            .name("memory-graph-roundtrip-seams".to_string())
            .stack_size(8 * 1024 * 1024)
            .spawn(move || {
                let config = Arc::new(openhuman_core::config::Config {
                    workspace_dir: workspace.clone(),
                    action_dir: workspace.clone(),
                    config_path: workspace.join("config.toml"),
                    ..openhuman_core::config::Config::default()
                });
                #[cfg(feature = "modules")]
                openhuman_core::modules::memory::set_modules_policy(config);
            })
            .expect("spawn memory graph seam installer")
            .join()
            .expect("memory graph seam installer panicked");
    });
}

fn ensure_rpc_auth() {
    AUTH_INIT.get_or_init(|| {
        // SAFETY: runs once, behind a `OnceLock`, before any server starts.
        unsafe { std::env::set_var(CORE_TOKEN_ENV_VAR, TEST_RPC_TOKEN) };
        let token_dir = std::env::temp_dir().join("openhuman-memory-graph-roundtrip-e2e-auth");
        init_rpc_token(&token_dir).expect("init rpc auth token");
    });
}

/// Config for a memory-backed core with no network reachability.
///
/// Note what is deliberately ABSENT: `[memory] provider = "none"`. Several
/// sibling suites set it, which binds the null driver — and the null driver
/// answers `graph_upsert` with "memory driver does not support the graph
/// family", so a suite that set it would assert nothing about a graph. The
/// default binding is the `tinymemory` module, which is what production uses
/// and what this file needs.
fn write_config(openhuman_dir: &Path) {
    std::fs::create_dir_all(openhuman_dir).expect("create .openhuman");
    let cfg = r#"api_url = "http://127.0.0.1:9"
default_model = "memory-graph-roundtrip-e2e-model"
default_temperature = 0.2

[secrets]
encrypt = false

[local_ai]
enabled = false

[memory_tree]
embedding_strict = false
"#;
    std::fs::write(openhuman_dir.join("config.toml"), cfg).expect("write config.toml");
    let _: openhuman_core::config::Config =
        toml::from_str(cfg).expect("test config must match schema");
}

struct Harness {
    rpc_base: String,
    _guards: Vec<EnvVarGuard>,
    join: tokio::task::JoinHandle<Result<(), std::io::Error>>,
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.join.abort();
    }
}

async fn setup() -> Harness {
    ensure_rpc_auth();

    let tmp = test_root();
    let home = tmp.path();
    write_config(&home.join(".openhuman"));
    let workspace = home.join("workspace");
    std::fs::create_dir_all(&workspace).expect("create workspace dir");
    ensure_memory_seams(&workspace);

    let guards = vec![
        EnvVarGuard::set_to_path("HOME", home),
        EnvVarGuard::set_to_path("OPENHUMAN_WORKSPACE", &workspace),
        EnvVarGuard::unset("BACKEND_URL"),
        EnvVarGuard::unset("VITE_BACKEND_URL"),
        EnvVarGuard::unset("OPENHUMAN_API_URL"),
        EnvVarGuard::set("OPENHUMAN_KEYRING_BACKEND", "file"),
        EnvVarGuard::set("OPENHUMAN_MEMORY_EMBED_STRICT", "false"),
        EnvVarGuard::set("OPENHUMAN_MEMORY_EMBED_ENDPOINT", ""),
        EnvVarGuard::set("OPENHUMAN_MEMORY_EMBED_MODEL", ""),
    ];

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind rpc listener");
    let addr = listener.local_addr().expect("rpc listener addr");
    let router = build_core_http_router(false);
    let join = tokio::spawn(async move { axum::serve(listener, router).await });

    Harness {
        rpc_base: format!("http://{addr}"),
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

/// Unwrap a JSON-RPC result, failing with the error body rather than a bare
/// `None` so a driver-level refusal names itself.
fn ok<'a>(value: &'a Value, context: &str) -> &'a Value {
    if let Some(error) = value.get("error") {
        panic!("{context}: unexpected JSON-RPC error: {error}");
    }
    value
        .get("result")
        .unwrap_or_else(|| panic!("{context}: missing result: {value}"))
}

/// Controllers wrap their payload in `{data|result}`; unwrap one level if present.
fn payload<'a>(value: &'a Value, context: &str) -> &'a Value {
    let outer = ok(value, context);
    outer
        .get("data")
        .or_else(|| outer.get("result"))
        .unwrap_or(outer)
}

fn rows<'a>(value: &'a Value, context: &str) -> &'a Vec<Value> {
    payload(value, context)
        .as_array()
        .unwrap_or_else(|| panic!("{context}: expected an array of relation rows: {value}"))
}

/// A namespace unique per run, so a shared workspace cannot make one test
/// observe another's rows and call it a pass.
fn unique_namespace(prefix: &str) -> String {
    format!(
        "{prefix}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    )
}

fn relation_matches(row: &Value, subject: &str, predicate: &str, object: &str) -> bool {
    let field = |name: &str| {
        row.get(name)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase()
    };
    // Case-insensitive on purpose: entity-name casing is the driver's policy,
    // not the contract's. TinyCortex upper-cases entity names, and pinning one
    // engine's normalisation from the host would fail a second driver that
    // preserved case — this test is about the wire path, not normalisation.
    field("subject") == subject.to_ascii_lowercase()
        && field("predicate") == predicate.to_ascii_lowercase()
        && field("object") == object.to_ascii_lowercase()
}

// ── Tests ────────────────────────────────────────────────────────────

/// 11.1.1 — the relation survives the whole wire path and comes back.
///
/// The unit test in `kv_graph_tests.rs` builds `GraphUpsertParams` as a struct
/// literal. This one posts JSON, so it is the only thing that exercises the
/// `#[serde(default)] Option<String>` namespace field, the dispatch table entry
/// and the response serialisation.
#[tokio::test]
async fn graph_relation_survives_the_json_rpc_round_trip() {
    let _lock = env_lock();
    let harness = setup().await;
    let namespace = unique_namespace("graph-rt");

    let upsert = rpc(
        &harness.rpc_base,
        1,
        "openhuman.memory_graph_upsert",
        json!({
            "namespace": namespace,
            "subject": "ALICE",
            "predicate": "WORKS_AT",
            "object": "ACME_CORP",
            "attrs": {"source": "memory-graph-roundtrip-e2e", "confidence": 0.9},
        }),
    )
    .await;
    assert_eq!(
        payload(&upsert, "memory_graph_upsert"),
        &json!(true),
        "memory_graph_upsert should report the relation was written"
    );

    let queried = rpc(
        &harness.rpc_base,
        2,
        "openhuman.memory_graph_query",
        json!({ "namespace": namespace }),
    )
    .await;
    let relations = rows(&queried, "memory_graph_query");

    assert!(
        relations
            .iter()
            .any(|row| relation_matches(row, "ALICE", "WORKS_AT", "ACME_CORP")),
        "memory_graph_query for namespace {namespace} should return the ALICE -[WORKS_AT]-> \
         ACME_CORP relation that memory_graph_upsert just wrote, but returned {relations:?}"
    );
}

/// 11.1.1 — a namespace-scoped query must not leak another namespace's rows.
///
/// This is the assertion the deleted `memory_graph_sync_e2e.rs` did NOT make.
/// It asserted only `graph_query_all >= graph_query_namespace`, which a driver
/// that ignored the namespace filter altogether would also satisfy — both
/// queries would return everything. Isolation is what actually breaks when the
/// filter breaks.
#[tokio::test]
async fn graph_query_does_not_leak_relations_across_namespaces() {
    let _lock = env_lock();
    let harness = setup().await;
    let mine = unique_namespace("graph-mine");
    let theirs = unique_namespace("graph-theirs");

    rpc(
        &harness.rpc_base,
        1,
        "openhuman.memory_graph_upsert",
        json!({
            "namespace": mine,
            "subject": "ALICE",
            "predicate": "WORKS_AT",
            "object": "ACME_CORP",
            "attrs": {},
        }),
    )
    .await;
    rpc(
        &harness.rpc_base,
        2,
        "openhuman.memory_graph_upsert",
        json!({
            "namespace": theirs,
            "subject": "MALLORY",
            "predicate": "WORKS_AT",
            "object": "OTHER_CORP",
            "attrs": {},
        }),
    )
    .await;

    let scoped = rpc(
        &harness.rpc_base,
        3,
        "openhuman.memory_graph_query",
        json!({ "namespace": mine }),
    )
    .await;
    let scoped_rows = rows(&scoped, "memory_graph_query(mine)");

    assert!(
        scoped_rows
            .iter()
            .any(|row| relation_matches(row, "ALICE", "WORKS_AT", "ACME_CORP")),
        "the namespace-scoped query should still return its own relation — \
         got {scoped_rows:?}"
    );
    assert!(
        !scoped_rows
            .iter()
            .any(|row| relation_matches(row, "MALLORY", "WORKS_AT", "OTHER_CORP")),
        "memory_graph_query scoped to {mine} leaked a relation stored under {theirs}; the \
         namespace filter is not being applied — got {scoped_rows:?}"
    );
}

/// 11.1.1 — the superset relationship the deleted suite pinned.
///
/// Kept because it is the documented relationship between a filtered and an
/// unfiltered query, and because it is the half that catches an unfiltered
/// query which silently scopes itself to something. On its own it is weak (see
/// the module docs); it is paired with the isolation test above, not a
/// substitute for it.
#[tokio::test]
async fn unfiltered_graph_query_is_a_superset_of_the_namespace_scoped_query() {
    let _lock = env_lock();
    let harness = setup().await;
    let namespace = unique_namespace("graph-superset");

    rpc(
        &harness.rpc_base,
        1,
        "openhuman.memory_graph_upsert",
        json!({
            "namespace": namespace,
            "subject": "ALICE",
            "predicate": "OWNS",
            "object": "ATLAS",
            "attrs": {},
        }),
    )
    .await;

    let scoped = rpc(
        &harness.rpc_base,
        2,
        "openhuman.memory_graph_query",
        json!({ "namespace": namespace }),
    )
    .await;
    let scoped_rows = rows(&scoped, "memory_graph_query(scoped)").clone();
    assert!(
        !scoped_rows.is_empty(),
        "the namespace-scoped query returned nothing, so the superset assertion below would \
         be vacuous — the upsert did not land"
    );

    // `namespace` omitted entirely, which is the `#[serde(default)]` path.
    let unfiltered = rpc(
        &harness.rpc_base,
        3,
        "openhuman.memory_graph_query",
        json!({}),
    )
    .await;
    let unfiltered_rows = rows(&unfiltered, "memory_graph_query(unfiltered)");

    assert!(
        unfiltered_rows.len() >= scoped_rows.len(),
        "an unfiltered memory_graph_query returned {} rows, fewer than the {} rows the query \
         scoped to {namespace} returned — the unfiltered read is scoping itself to something",
        unfiltered_rows.len(),
        scoped_rows.len()
    );
    assert!(
        unfiltered_rows
            .iter()
            .any(|row| relation_matches(row, "ALICE", "OWNS", "ATLAS")),
        "the unfiltered query should contain the relation the scoped query found — \
         got {unfiltered_rows:?}"
    );
}
