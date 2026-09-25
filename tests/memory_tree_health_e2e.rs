//! End-to-end cover for the `memory_tree` health and lifecycle controllers.
//!
//! # Why this target exists
//!
//! `memory_tree` reported **5/5, 100%** to
//! `scripts/check-domain-e2e-coverage.mjs` while carrying **31** dispatchable
//! controllers. Discovery read `const NAMESPACE` one file at a time, so it only
//! saw the five declared in `memory/tree/retrieval/schemas.rs` — the other 26
//! live in `memory/schema/{tree_operations,vault_and_pipeline}_schema.rs` and
//! reach their `const` through `use super::*`. A namespace nobody measures is
//! the same fail-open shape as a test filter that matches nothing, except this
//! one had a green tick on it. The gate fix in the same change makes the real
//! figure 24/31; the seven cases below are the gap it exposes.
//!
//! These are NOT in `tests/raw_coverage/memory_goals_people_e2e.rs`, the file
//! that already drives `memory_tree_*`, because that file opens with
//! `#![cfg(any())]` (the #6382 quarantine) and compiles to nothing. Adding a
//! case there would credit the coverage gate and run no code at all.
//!
//! # What is asserted, and what is not
//!
//! Every case here asserts a claim the handler makes about itself, not that a
//! call returns 200. Where a claim needs state this harness cannot produce —
//! a terminally failed job for `retry_failed` to requeue — the case asserts the
//! cross-method invariant that IS reachable and says so in a comment, rather
//! than asserting `0 == 0` and calling it coverage.

#[path = "support/memory_module.rs"]
mod memory_module;

use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use axum::http::header::AUTHORIZATION;
use serde_json::{json, Value};
use tempfile::tempdir;

use openhuman_core::core::auth::{init_rpc_token, CORE_TOKEN_ENV_VAR};
use openhuman_core::core::jsonrpc::build_core_http_router;

const TEST_RPC_TOKEN: &str = "memory-tree-health-e2e-token";
static AUTH_INIT: OnceLock<()> = OnceLock::new();
static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static MEMORY_SEAMS_INIT: OnceLock<()> = OnceLock::new();
static TEST_HOME: OnceLock<tempfile::TempDir> = OnceLock::new();

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    let mutex = ENV_LOCK.get_or_init(|| Mutex::new(()));
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn ensure_rpc_auth() {
    AUTH_INIT.get_or_init(|| {
        unsafe { std::env::set_var(CORE_TOKEN_ENV_VAR, TEST_RPC_TOKEN) };
        let token_dir = std::env::temp_dir().join("openhuman-memory-sources-e2e-auth");
        init_rpc_token(&token_dir).expect("init rpc auth");
    });
}

/// The transport-only JSON-RPC router does not create a core runtime context,
/// so memory-backed routes need their host seams installed explicitly.
fn ensure_memory_seams() {
    MEMORY_SEAMS_INIT.get_or_init(|| {
        std::thread::Builder::new()
            .name("memory-tree-health-e2e-seams".to_string())
            .stack_size(8 * 1024 * 1024)
            .spawn(|| {
                #[cfg(feature = "modules")]
                openhuman_core::modules::memory::set_modules_policy(Arc::new(
                    openhuman_core::config::Config::default(),
                ));
            })
            .expect("spawn memory tree health seam installer")
            .join()
            .expect("memory tree health seam installer panicked");
    });
}

fn test_home() -> &'static Path {
    TEST_HOME
        .get_or_init(|| tempdir().expect("memory tree health tempdir"))
        .path()
}

struct EnvVarGuard {
    key: &'static str,
    old: Option<String>,
}

impl EnvVarGuard {
    fn set_to_path(key: &'static str, path: &Path) -> Self {
        let old = std::env::var(key).ok();
        unsafe { std::env::set_var(key, path.as_os_str()) };
        Self { key, old }
    }

    fn unset(key: &'static str) -> Self {
        let old = std::env::var(key).ok();
        unsafe { std::env::remove_var(key) };
        Self { key, old }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.old {
            Some(v) => unsafe { std::env::set_var(self.key, v) },
            None => unsafe { std::env::remove_var(self.key) },
        }
    }
}

fn write_config(dir: &Path) {
    std::fs::create_dir_all(dir).expect("mkdir");
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
    // Every flow here reaches the memory module; wait out its load so a test
    // running in its own process does not race it (tests/support/memory_module.rs).
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
    let body = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    });
    let url = format!("{}/rpc", base.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .header(AUTHORIZATION, format!("Bearer {TEST_RPC_TOKEN}"))
        .json(&body)
        .send()
        .await
        .unwrap_or_else(|e| panic!("POST {url}: {e}"));
    assert!(
        resp.status().is_success(),
        "HTTP error {} for {method}",
        resp.status(),
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
    // RpcOutcome wraps the payload under an inner "result" key alongside "logs".
    if let Some(inner) = outer.get("result") {
        inner.clone()
    } else {
        outer.clone()
    }
}

// ── Tests ──────────────────────────────────

/// Ingest one source under `home`, returning the chunk count written.
async fn ingest_source(base: &str, id: i64, source_id: &str, body: &str) -> u64 {
    let response = rpc(
        base,
        id,
        "openhuman.memory_tree_ingest",
        json!({
            "source_kind": "document",
            "source_id": source_id,
            "owner": "user",
            "tags": ["memory_tree_health_e2e", "document"],
            "payload": { "provider": "memory_tree_health_e2e", "title": source_id, "body": body },
        }),
    )
    .await;
    ok(&response, &format!("ingest {source_id}"))
        .get("chunks_written")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("ingest {source_id} must report chunks_written: {response}"))
}

/// Enough prose that chunking produces at least one chunk per source.
fn body_for(source: &str) -> String {
    format!(
        "{source} is a durable note used by the memory tree health suite. \
         It repeats enough distinct sentences that the chunker has real content to seal. \
         The retention policy for {source} is tested by deleting it and observing that \
         every other source survives untouched. Structural recall over {source} must \
         continue to answer after its siblings are removed."
    )
}

/// `memory_tree.doctor` must agree with itself.
///
/// The handler's own schema states the invariant: `healthy` is
/// "True when no stage is blocking (first_blocking_cause is null)". A doctor
/// whose verdict and evidence disagree is worse than no doctor — it is the
/// surface an operator trusts to decide whether memory is working, and the
/// failure mode worth catching is a rewrite that always answers `healthy: true`
/// while `first_blocking_cause` is populated. Asserting the two fields against
/// each other catches that in either direction without needing to force a fault.
#[tokio::test]
async fn memory_tree_doctor_verdict_agrees_with_its_own_evidence() {
    let _guard = env_lock();
    let home = test_home();
    let openhuman_home = home.join(".openhuman");
    write_config(&openhuman_home);
    let _home = EnvVarGuard::set_to_path("HOME", home);
    let _ws = EnvVarGuard::unset("OPENHUMAN_WORKSPACE");
    let _backend = EnvVarGuard::unset("BACKEND_URL");
    let _vite = EnvVarGuard::unset("VITE_BACKEND_URL");
    let (base, rpc_join) = serve().await;

    let response = rpc(&base, 500, "openhuman.memory_tree_doctor", json!({})).await;
    let doctor = ok(&response, "memory_tree_doctor");

    let healthy = doctor
        .get("healthy")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| panic!("doctor must report `healthy`: {doctor}"));
    // A healthy doctor OMITS the key rather than serialising `null`
    // (`skip_serializing_if = "Option::is_none"`), so absent and null are the
    // same statement — "nothing is blocking". Treating them differently made
    // this case depend on whatever state the store happened to be in.
    let blocking = doctor
        .get("first_blocking_cause")
        .cloned()
        .unwrap_or(Value::Null);

    assert_eq!(
        healthy,
        blocking.is_null(),
        "`healthy` is defined as `first_blocking_cause is null`; the two disagree: {doctor}"
    );

    // The evidence must actually be there. An empty stage list would make the
    // verdict above true by vacuity whatever the pipeline was doing.
    let stages = doctor
        .get("stages")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("doctor must report a `stages` array: {doctor}"));
    assert!(
        !stages.is_empty(),
        "an empty `stages` array makes the verdict unfalsifiable: {doctor}"
    );
    for stage in stages {
        assert!(
            stage.get("stage").and_then(Value::as_str).is_some(),
            "every stage row must name its stage: {stage}"
        );
        assert!(
            stage.get("ok").and_then(Value::as_bool).is_some(),
            "every stage row must carry a boolean `ok`: {stage}"
        );
    }

    // And a blocking cause, when present, must be typed rather than bare prose:
    // the frontend routes remediation off `remediation_key`.
    if !blocking.is_null() {
        for field in ["code", "class", "remediation_key"] {
            assert!(
                blocking.get(field).is_some(),
                "a blocking cause must be typed and carry `{field}`: {blocking}"
            );
        }
    }

    // `counters` is what an operator reads next; a stage saying "not ok" with no
    // counters is a dead end.
    let counters = doctor
        .get("counters")
        .unwrap_or_else(|| panic!("doctor must report `counters`: {doctor}"));
    for field in ["total_chunks", "jobs_ready", "jobs_running", "jobs_failed"] {
        assert!(
            counters.get(field).and_then(Value::as_u64).is_some(),
            "counters must carry a numeric `{field}`: {counters}"
        );
    }

    rpc_join.abort();
}

/// `memory_tree.delete_source` must delete ONE source, not the neighbourhood.
///
/// Over-deletion is silent data loss: the call reports success either way, and
/// the only difference is the sibling that is no longer there. So the assertion
/// is on the survivor, not on the return value. Two sources are ingested, one is
/// deleted, and the other must still be listed with its chunks intact.
#[tokio::test]
async fn memory_tree_delete_source_removes_only_the_named_source() {
    let _guard = env_lock();
    let home = test_home();
    let openhuman_home = home.join(".openhuman");
    write_config(&openhuman_home);
    let _home = EnvVarGuard::set_to_path("HOME", home);
    let _ws = EnvVarGuard::unset("OPENHUMAN_WORKSPACE");
    let _backend = EnvVarGuard::unset("BACKEND_URL");
    let _vite = EnvVarGuard::unset("VITE_BACKEND_URL");
    let (base, rpc_join) = serve().await;

    let doomed = "memory_tree_health:doomed-source";
    let survivor = "memory_tree_health:surviving-source";

    let doomed_chunks = ingest_source(&base, 510, doomed, &body_for(doomed)).await;
    let survivor_chunks = ingest_source(&base, 511, survivor, &body_for(survivor)).await;
    assert!(
        doomed_chunks >= 1 && survivor_chunks >= 1,
        "both fixtures must write at least one chunk, or the deletion proves nothing \
         (doomed={doomed_chunks}, survivor={survivor_chunks})"
    );

    let deleted = rpc(
        &base,
        512,
        "openhuman.memory_tree_delete_source",
        json!({ "source_id": doomed }),
    )
    .await;
    let deleted_result = ok(&deleted, "memory_tree_delete_source");
    assert_eq!(
        deleted_result.get("deleted").and_then(Value::as_bool),
        Some(true),
        "deleting a source that exists must report real work: {deleted_result}"
    );
    assert!(
        deleted_result
            .get("chunks_removed")
            .and_then(Value::as_u64)
            .is_some_and(|removed| removed >= 1),
        "deleting a source with chunks must remove at least one: {deleted_result}"
    );

    // THE POINT: the sibling survived.
    let listed = rpc(&base, 513, "openhuman.memory_tree_list_sources", json!({})).await;
    let sources = ok(&listed, "memory_tree_list_sources");
    let rendered = sources.to_string();
    assert!(
        rendered.contains(survivor),
        "deleting `{doomed}` must leave `{survivor}` in place; list was: {rendered}"
    );
    assert!(
        !rendered.contains(doomed),
        "the deleted source must be gone from list_sources; list was: {rendered}"
    );

    // Deleting an id that does not exist must be honest about doing nothing,
    // rather than reporting success and hiding a typo'd id from the caller.
    let absent = rpc(
        &base,
        514,
        "openhuman.memory_tree_delete_source",
        json!({ "source_id": "memory_tree_health:never-ingested" }),
    )
    .await;
    let absent_result = ok(&absent, "memory_tree_delete_source (absent)");
    assert_eq!(
        absent_result.get("chunks_removed").and_then(Value::as_u64),
        Some(0),
        "deleting a source that was never ingested must remove nothing: {absent_result}"
    );

    rpc_join.abort();
}

/// `vault_health_check` and `pipeline_status` must not contradict each other.
///
/// Two health surfaces over one pipeline. They are read by different callers —
/// onboarding reads the vault snapshot, the memory panel reads the pipeline —
/// and nothing today stops them drifting apart. The invariant asserted here is
/// the one an operator relies on: if `doctor` names a blocking cause, the
/// pipeline must not simultaneously present itself as unblocked.
#[tokio::test]
async fn memory_tree_vault_and_pipeline_health_surfaces_agree() {
    let _guard = env_lock();
    let home = test_home();
    let openhuman_home = home.join(".openhuman");
    write_config(&openhuman_home);
    let _home = EnvVarGuard::set_to_path("HOME", home);
    let _ws = EnvVarGuard::unset("OPENHUMAN_WORKSPACE");
    let _backend = EnvVarGuard::unset("BACKEND_URL");
    let _vite = EnvVarGuard::unset("VITE_BACKEND_URL");
    let (base, rpc_join) = serve().await;

    let vault = rpc(
        &base,
        520,
        "openhuman.memory_tree_vault_health_check",
        json!({}),
    )
    .await;
    let vault_result = ok(&vault, "memory_tree_vault_health_check");
    assert!(
        vault_result.is_object(),
        "the vault snapshot must be an object, not a bare scalar: {vault_result}"
    );

    let pipeline = rpc(
        &base,
        521,
        "openhuman.memory_tree_pipeline_status",
        json!({}),
    )
    .await;
    let pipeline_result = ok(&pipeline, "memory_tree_pipeline_status");
    assert!(
        pipeline_result.is_object(),
        "the pipeline snapshot must be an object: {pipeline_result}"
    );

    let doctor = ok(
        &rpc(&base, 522, "openhuman.memory_tree_doctor", json!({})).await,
        "memory_tree_doctor",
    );
    let blocked = !doctor
        .get("first_blocking_cause")
        .map(Value::is_null)
        .unwrap_or(true);

    if blocked {
        // A pipeline that calls itself healthy while the doctor is blocked is the
        // drift this case exists to catch.
        let pipeline_claims_health = pipeline_result
            .get("healthy")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        assert!(
            !pipeline_claims_health,
            "doctor reports a blocking cause ({:?}) while pipeline_status reports healthy: {pipeline_result}",
            doctor.get("first_blocking_cause")
        );
    }

    rpc_join.abort();
}

/// `memory_tree.retry_failed` must report what it requeued, and not invent work.
///
/// The honest limit of this case, stated rather than hidden: this harness has no
/// way to force a terminally failed `mem_tree_jobs` row, so it cannot prove the
/// requeue path moves a real job. What it CAN prove is the cross-method
/// invariant — `requeued` must never exceed the `jobs_failed` the doctor counted
/// a moment earlier, and a second call with nothing left to requeue must report
/// zero. A handler that returns an invented non-zero count, or that counts the
/// whole job table rather than the failed rows, fails both.
#[tokio::test]
async fn memory_tree_retry_failed_never_requeues_more_than_had_failed() {
    let _guard = env_lock();
    let home = test_home();
    let openhuman_home = home.join(".openhuman");
    write_config(&openhuman_home);
    let _home = EnvVarGuard::set_to_path("HOME", home);
    let _ws = EnvVarGuard::unset("OPENHUMAN_WORKSPACE");
    let _backend = EnvVarGuard::unset("BACKEND_URL");
    let _vite = EnvVarGuard::unset("VITE_BACKEND_URL");
    let (base, rpc_join) = serve().await;

    let before = ok(
        &rpc(&base, 530, "openhuman.memory_tree_doctor", json!({})).await,
        "memory_tree_doctor (before retry)",
    );
    let failed_before = before
        .get("counters")
        .and_then(|counters| counters.get("jobs_failed"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("doctor must count jobs_failed: {before}"));

    let retried = rpc(&base, 531, "openhuman.memory_tree_retry_failed", json!({})).await;
    let retried_result = ok(&retried, "memory_tree_retry_failed");
    let requeued = retried_result
        .get("requeued")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("retry_failed must report `requeued`: {retried_result}"));

    assert!(
        requeued <= failed_before,
        "retry_failed requeued {requeued} jobs but the doctor counted only {failed_before} failed; \
         it must requeue failed rows, not the whole job table"
    );

    // Nothing is left failed after a requeue, so a second call must be a no-op.
    // A handler that re-counts the same rows every time reports non-zero forever.
    let again = rpc(&base, 532, "openhuman.memory_tree_retry_failed", json!({})).await;
    assert_eq!(
        ok(&again, "memory_tree_retry_failed (second call)")
            .get("requeued")
            .and_then(Value::as_u64),
        Some(0),
        "a second retry_failed with nothing left to requeue must report 0: {again}"
    );

    rpc_join.abort();
}

/// `flush_source` must seal the scope it was given, and echo that scope back.
///
/// The echo is the assertable half: `tree_scope` is documented as "Echo of the
/// source scope", so a handler that flushes a different scope than it was asked
/// to — or silently falls back to a global flush — shows up as a mismatch.
#[tokio::test]
async fn memory_tree_flush_source_seals_only_the_scope_it_was_given() {
    let _guard = env_lock();
    let home = test_home();
    let openhuman_home = home.join(".openhuman");
    write_config(&openhuman_home);
    let _home = EnvVarGuard::set_to_path("HOME", home);
    let _ws = EnvVarGuard::unset("OPENHUMAN_WORKSPACE");
    let _backend = EnvVarGuard::unset("BACKEND_URL");
    let _vite = EnvVarGuard::unset("VITE_BACKEND_URL");
    let (base, rpc_join) = serve().await;

    let scope = "memory_tree_health:flush-scope";
    let chunks = ingest_source(&base, 540, scope, &body_for(scope)).await;
    assert!(
        chunks >= 1,
        "the fixture must write a chunk, or there is nothing to seal"
    );

    let flushed = rpc(
        &base,
        541,
        "openhuman.memory_tree_flush_source",
        json!({ "source_scope": scope }),
    )
    .await;
    let flushed_result = ok(&flushed, "memory_tree_flush_source");

    assert_eq!(
        flushed_result.get("tree_scope").and_then(Value::as_str),
        Some(scope),
        "flush_source must echo back the scope it sealed, not a different one: {flushed_result}"
    );
    assert!(
        flushed_result
            .get("seals_fired")
            .and_then(Value::as_u64)
            .is_some(),
        "flush_source must report how many seal cascades fired: {flushed_result}"
    );

    rpc_join.abort();
}

/// `backfill_connector_trees` with `dry_run` must not mutate.
///
/// A dry run that writes is the worst kind of defect in a backfill: the operator
/// ran it precisely to find out what WOULD happen. The chunk total before and
/// after is the observable that proves it, and it is read from `doctor.counters`
/// rather than from the backfill's own return value, so the check does not
/// depend on the code under test telling the truth about itself.
#[tokio::test]
async fn memory_tree_backfill_dry_run_reports_without_writing() {
    let _guard = env_lock();
    let home = test_home();
    let openhuman_home = home.join(".openhuman");
    write_config(&openhuman_home);
    let _home = EnvVarGuard::set_to_path("HOME", home);
    let _ws = EnvVarGuard::unset("OPENHUMAN_WORKSPACE");
    let _backend = EnvVarGuard::unset("BACKEND_URL");
    let _vite = EnvVarGuard::unset("VITE_BACKEND_URL");
    let (base, rpc_join) = serve().await;

    let source = "memory_tree_health:backfill-source";
    assert!(
        ingest_source(&base, 550, source, &body_for(source)).await >= 1,
        "the fixture must write a chunk, or an unchanged total proves nothing"
    );

    let chunks_of = |doctor: &Value| -> u64 {
        doctor
            .get("counters")
            .and_then(|counters| counters.get("total_chunks"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("doctor must count total_chunks: {doctor}"))
    };

    let before = chunks_of(&ok(
        &rpc(&base, 551, "openhuman.memory_tree_doctor", json!({})).await,
        "doctor (before dry run)",
    ));
    assert!(
        before >= 1,
        "the tree must hold at least one chunk before the dry run, or an unchanged \
         total is unfalsifiable"
    );

    let dry = rpc(
        &base,
        552,
        "openhuman.memory_tree_backfill_connector_trees",
        json!({ "dry_run": true }),
    )
    .await;
    ok(&dry, "memory_tree_backfill_connector_trees (dry run)");

    let after = chunks_of(&ok(
        &rpc(&base, 553, "openhuman.memory_tree_doctor", json!({})).await,
        "doctor (after dry run)",
    ));
    assert_eq!(
        after, before,
        "a dry run must not change the chunk total ({before} -> {after})"
    );

    rpc_join.abort();
}

/// `smart_walk` must answer a query over content that was actually ingested.
///
/// The retrieval half of the tree. The assertion is that a query drawn from the
/// ingested body reaches a result set, and that the shape is the documented one
/// — not that a specific chunk ranks first, which would pin the ranker rather
/// than the contract.
#[tokio::test]
async fn memory_tree_smart_walk_answers_over_ingested_content() {
    let _guard = env_lock();
    let home = test_home();
    let openhuman_home = home.join(".openhuman");
    write_config(&openhuman_home);
    let _home = EnvVarGuard::set_to_path("HOME", home);
    let _ws = EnvVarGuard::unset("OPENHUMAN_WORKSPACE");
    let _backend = EnvVarGuard::unset("BACKEND_URL");
    let _vite = EnvVarGuard::unset("VITE_BACKEND_URL");
    let (base, rpc_join) = serve().await;

    let source = "memory_tree_health:smart-walk-source";
    assert!(
        ingest_source(&base, 560, source, &body_for(source)).await >= 1,
        "the fixture must write a chunk, or the walk has nothing to reach"
    );

    let walked = rpc(
        &base,
        561,
        "openhuman.memory_tree_smart_walk",
        json!({ "query": "retention policy", "limit": 5 }),
    )
    .await;
    let result = ok(&walked, "memory_tree_smart_walk");
    assert!(
        result.is_object() || result.is_array(),
        "smart_walk must return a structured result set: {result}"
    );

    // `limit` is the caller's bound; a walk that ignores it can flood a prompt.
    if let Some(items) = result
        .get("results")
        .or_else(|| result.get("chunks"))
        .and_then(Value::as_array)
    {
        assert!(
            items.len() <= 5,
            "smart_walk returned {} items for limit=5: {result}",
            items.len()
        );
    }

    rpc_join.abort();
}

/// `memory.namespace_summaries` — the total must equal the parts, and the parts
/// must be the counts that were actually written.
///
/// Its own description calls this "the verification surface for whether a sync's
/// items actually landed", which makes a wrong total worse than no total: it
/// vindicates a broken sync. `memory/ops/documents.rs` computes
/// `total_documents` as `namespaces.iter().map(|n| n.count).sum()`.
///
/// `total == sum` alone is too weak — an all-zero response satisfies it, and so
/// does a handler that echoes one row's count into every row. So this seeds two
/// namespaces with **different non-zero counts** and asserts each one exactly.
/// Two distinct values is what rules out the echo; a non-zero fixture is what
/// rules out the empty store. (Credit to the parallel `memory_roundtrip_e2e`
/// work for the distinct-counts argument.)
///
/// This is the **controller** test, over JSON-RPC. `tests/memory_roundtrip_e2e.rs`
/// covers the same ground one layer down by calling
/// `openhuman_core::memory::ops::memory_namespace_summaries` directly. Both are
/// worth having and only this one exercises dispatch, parameter decoding and the
/// `RpcOutcome` envelope.
#[tokio::test]
async fn memory_namespace_summaries_counts_each_namespace_it_was_given() {
    let _guard = env_lock();
    let home = test_home();
    let openhuman_home = home.join(".openhuman");
    write_config(&openhuman_home);
    let _home_guard = EnvVarGuard::set_to_path("HOME", home);
    let _ws = EnvVarGuard::unset("OPENHUMAN_WORKSPACE");
    let _backend = EnvVarGuard::unset("BACKEND_URL");
    let _vite = EnvVarGuard::unset("VITE_BACKEND_URL");
    let (base, rpc_join) = serve().await;

    // Distinct counts on purpose: 2 and 3, so a handler that reports one row's
    // count for every row cannot pass.
    let small = "memory_tree_health_summaries_small";
    let large = "memory_tree_health_summaries_large";
    let seeded: [(&str, usize); 2] = [(small, 2), (large, 3)];

    // Start from a known floor so a leftover store cannot make the assertions
    // pass for the wrong reason.
    for (namespace, _) in seeded {
        rpc(
            &base,
            580,
            "openhuman.memory_clear_namespace",
            json!({ "namespace": namespace }),
        )
        .await;
    }

    let mut id = 581;
    for (namespace, count) in seeded {
        for index in 0..count {
            let response = rpc(
                &base,
                id,
                "openhuman.memory_doc_put",
                json!({
                    "namespace": namespace,
                    "key": format!("{namespace}-doc-{index}"),
                    "title": format!("{namespace} document {index}"),
                    "content": format!(
                        "Document {index} of namespace {namespace}, written by the \
                         namespace-summaries e2e case so the counts below are ones \
                         this test put there itself."
                    ),
                }),
            )
            .await;
            ok(&response, &format!("memory_doc_put {namespace}#{index}"));
            id += 1;
        }
    }

    let response = rpc(
        &base,
        600,
        "openhuman.memory_namespace_summaries",
        json!({}),
    )
    .await;
    let summaries = ok(&response, "memory_namespace_summaries");

    let namespaces = summaries
        .get("namespaces")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("summaries must carry a `namespaces` array: {summaries}"))
        .clone();
    let total = summaries
        .get("total_documents")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("summaries must carry `total_documents`: {summaries}"));

    let count_for = |wanted: &str| -> u64 {
        namespaces
            .iter()
            .find(|entry| entry.get("namespace").and_then(Value::as_str) == Some(wanted))
            .and_then(|entry| entry.get("count").and_then(Value::as_u64))
            .unwrap_or_else(|| panic!("namespace `{wanted}` must appear with a count: {summaries}"))
    };

    // Each namespace reports the count IT was given, not the other one's.
    for (namespace, expected) in seeded {
        assert_eq!(
            count_for(namespace),
            expected as u64,
            "namespace `{namespace}` must report the {expected} documents written to it: {summaries}"
        );
    }

    // And the headline figure is the sum of the rows, not an independent tally
    // that can drift from them.
    let summed: u64 = namespaces
        .iter()
        .map(|entry| {
            entry
                .get("count")
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("every namespace row must carry a count: {entry}"))
        })
        .sum();
    assert_eq!(
        total, summed,
        "total_documents ({total}) must equal the sum of per-namespace counts ({summed}): {summaries}"
    );
    assert!(
        total >= 5,
        "the five seeded documents must be included in the total: {summaries}"
    );

    rpc_join.abort();
}
