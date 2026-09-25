//! Memory subsystem round-trip integration test (#773 PR-A).
//!
//! Validates the full doc_put → recall_memories → clear_namespace lifecycle
//! against a real local memory client backed by the workspace store under a
//! per-test temp `OPENHUMAN_WORKSPACE`.
//!
//! Counterpart to `app/test/e2e/specs/memory-roundtrip.spec.ts` which exercises
//! the same flow over JSON-RPC. This Rust test verifies the Rust contract in
//! isolation; the WDIO spec proves the UI⇄Tauri⇄sidecar wiring.
//!
//! # Running this target
//!
//! ```text
//! RUST_MIN_STACK=67108864 cargo test -p openhuman-cli \
//!   --features "$(bash scripts/ci/product-features.sh)" \
//!   --test memory_roundtrip_e2e
//! ```
//!
//! Both parts are load-bearing and both fail in ways that look like product
//! bugs rather than invocation mistakes:
//!
//! * **`--features`.** `openhuman-cli`'s own `modules` flag is NOT in its
//!   `default` set, even though `openhuman-core/modules` is switched on
//!   transitively. A `#[cfg(feature = "modules")]` block inside an integration
//!   test is evaluated against **openhuman-cli**'s features, so without this
//!   the `set_modules_policy` call in `ensure_memory_seams` below compiles to
//!   nothing and every module-backed call fails with "the module host policy
//!   was never published, so module 'tinymemory' cannot be loaded". The seam
//!   becoming a silent no-op is the worst shape for that trap: the only hint is
//!   an `unused variable: config` warning. (Diagnosed by the W6/`tinyhuman-a8`
//!   pass, 2026-09-24.)
//! * **`RUST_MIN_STACK`.** Without it the namespace-summaries test overflows its
//!   stack and SIGABRTs before reaching an assertion — the #5021 class, and the
//!   value the rust-gates-off lane already sets.
//!
//! `scripts/test-rust-e2e.sh` supplies both; a bare
//! `cargo test --test memory_roundtrip_e2e` supplies neither.

use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use tempfile::tempdir;

use openhuman_core::memory::ops::{
    clear_namespace, doc_put, memory_namespace_summaries, memory_recall_context,
    memory_recall_memories, ClearNamespaceParams, PutDocParams,
};
use openhuman_core::memory::rpc_models::{RecallContextRequest, RecallMemoriesRequest};

// ── Env isolation ────────────────────────────────────────────────────

struct EnvVarGuard {
    key: &'static str,
    old: Option<String>,
}

impl EnvVarGuard {
    fn set_to_path(key: &'static str, path: &Path) -> Self {
        let old = std::env::var(key).ok();
        // SAFETY: EnvVarGuard is only used in tests that first acquire
        // env_lock(), which serializes process-global env mutations.
        unsafe { std::env::set_var(key, path.as_os_str()) };
        Self { key, old }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.old {
            // SAFETY: See EnvVarGuard::set_to_path; teardown runs under the same
            // env_lock() critical section as setup.
            Some(v) => unsafe { std::env::set_var(self.key, v) },
            // SAFETY: Guarded by env_lock(), preventing concurrent env access.
            None => unsafe { std::env::remove_var(self.key) },
        }
    }
}

/// Serialises tests: `HOME` + `OPENHUMAN_WORKSPACE` are process-global.
static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static MEMORY_SEAMS_INIT: OnceLock<()> = OnceLock::new();
static TEST_ROOT: OnceLock<tempfile::TempDir> = OnceLock::new();

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    match ENV_LOCK.get_or_init(|| Mutex::new(())).lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// This integration target calls memory operations without constructing a core
/// runtime, so it supplies the seams that normal startup installs first.
fn test_root() -> &'static tempfile::TempDir {
    TEST_ROOT.get_or_init(|| tempdir().expect("memory roundtrip tempdir"))
}

fn ensure_memory_seams(workspace: &Path) {
    MEMORY_SEAMS_INIT.get_or_init(|| {
        let workspace = workspace.to_path_buf();
        std::thread::Builder::new()
            .name("memory-roundtrip-seams".to_string())
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
            .expect("spawn memory roundtrip seam installer")
            .join()
            .expect("memory roundtrip seam installer panicked");
    });
}

const NS: &str = "memory-roundtrip-e2e-773";
const KEY: &str = "roundtrip-canary-key";
const TITLE: &str = "Memory roundtrip canary";
const CONTENT: &str = "OpenHuman memory roundtrip canary fact #773";

fn put_params() -> PutDocParams {
    PutDocParams {
        namespace: NS.to_string(),
        key: KEY.to_string(),
        title: TITLE.to_string(),
        content: CONTENT.to_string(),
        source_type: "doc".to_string(),
        priority: "medium".to_string(),
        tags: Vec::new(),
        metadata: serde_json::Value::Null,
        category: "core".to_string(),
        session_id: None,
        document_id: None,
    }
}

fn recall_request() -> RecallMemoriesRequest {
    RecallMemoriesRequest {
        namespace: NS.to_string(),
        min_retention: None,
        as_of: None,
        limit: Some(10),
        max_chunks: None,
        top_k: None,
    }
}

fn recall_context_request() -> RecallContextRequest {
    RecallContextRequest {
        namespace: NS.to_string(),
        include_references: Some(true),
        limit: Some(10),
        max_chunks: None,
    }
}

// ── Tests ────────────────────────────────────────────────────────────

/// 8.1.1 store + 8.1.2 recall — the happy-path round-trip.
#[tokio::test]
async fn doc_put_then_recall_memories_returns_canary() {
    let _lock = env_lock();
    let tmp = test_root();
    let _home = EnvVarGuard::set_to_path("HOME", tmp.path());
    let workspace_path = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace_path).expect("create workspace dir");
    let _ws = EnvVarGuard::set_to_path("OPENHUMAN_WORKSPACE", &workspace_path);
    ensure_memory_seams(&workspace_path);

    // Store the canary document.
    let put_outcome = doc_put(put_params()).await.expect("doc_put rpc");
    assert!(
        !put_outcome.value.document_id.is_empty(),
        "doc_put should return a non-empty document_id"
    );

    // Recall the namespace and assert the canary surface.
    let recall_outcome = memory_recall_memories(recall_request())
        .await
        .expect("memory_recall_memories rpc");
    let serialised =
        serde_json::to_string(&recall_outcome.value).expect("serialise recall envelope");
    assert!(
        serialised.contains(CONTENT) || serialised.contains(KEY),
        "recall payload should reference the canary content/key — got {serialised}"
    );
}

/// `recall_context` should surface the same document as an LLM-ready prompt
/// block, not only in the raw memory list view.
#[tokio::test]
async fn doc_put_then_recall_context_renders_llm_context_message() {
    let _lock = env_lock();
    let tmp = test_root();
    let _home = EnvVarGuard::set_to_path("HOME", tmp.path());
    let workspace_path = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace_path).expect("create workspace dir");
    let _ws = EnvVarGuard::set_to_path("OPENHUMAN_WORKSPACE", &workspace_path);
    ensure_memory_seams(&workspace_path);

    doc_put(put_params()).await.expect("doc_put rpc");

    let recall_outcome = memory_recall_context(recall_context_request())
        .await
        .expect("memory_recall_context rpc");
    let llm_context = recall_outcome
        .value
        .data
        .as_ref()
        .and_then(|data| data.llm_context_message.as_ref())
        .cloned()
        .unwrap_or_default();
    assert!(
        llm_context.contains(CONTENT) || llm_context.contains(KEY),
        "llm context should reference the canary content/key — got {llm_context}"
    );
}

/// doc_put with a body whose multi-byte codepoint straddles the 2048-byte
/// body_preview boundary must complete without panic and return a non-empty
/// document_id (PR #1681 regression guard).
///
/// Scenario: a ZWNJ (U+200C, 3 bytes: 0xE2 0x80 0x8C) is placed so each of
/// its bytes falls exactly on the nominal 2048-byte cut point in turn.
/// The ingest path calls `markdown_body_preview` which uses `ceil_char_boundary`
/// — a panic here would surface as a test failure.
#[tokio::test]
async fn doc_put_with_multibyte_at_body_preview_boundary_does_not_panic() {
    let _lock = env_lock();
    let tmp = test_root();
    let _home = EnvVarGuard::set_to_path("HOME", tmp.path());
    let workspace_path = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace_path).expect("create workspace dir");
    let _ws = EnvVarGuard::set_to_path("OPENHUMAN_WORKSPACE", &workspace_path);
    ensure_memory_seams(&workspace_path);

    const BODY_PREVIEW_MAX_BYTES: usize = 2048;
    let zwnj = '\u{200c}'; // 3-byte codepoint
    let zwnj_bytes = zwnj.len_utf8();

    for offset in 0..zwnj_bytes {
        // Build a body where the nominal cut falls exactly `offset` bytes into the
        // ZWNJ. `prefix_len` bytes of 'a' are placed before the ZWNJ so that the
        // 2048-byte cut point lands `offset` bytes into the 3-byte ZWNJ codepoint.
        // Total body length is prefix_len + zwnj_bytes + trailing, which is
        // > BODY_PREVIEW_MAX_BYTES since trailing = offset + 80 >= 80.
        let prefix_len = BODY_PREVIEW_MAX_BYTES - offset;
        let body = format!(
            "{}{}{}",
            "a".repeat(prefix_len),
            zwnj,
            "b".repeat(offset + 80)
        );
        assert!(
            body.len() > BODY_PREVIEW_MAX_BYTES,
            "offset={offset}: fixture body too short to exercise truncation"
        );

        let params = PutDocParams {
            namespace: format!("utf8-boundary-e2e-{offset}"),
            key: format!("utf8-boundary-key-{offset}"),
            title: format!("UTF-8 boundary test offset={offset}"),
            content: body,
            source_type: "doc".to_string(),
            priority: "medium".to_string(),
            tags: Vec::new(),
            metadata: serde_json::Value::Null,
            category: "core".to_string(),
            session_id: None,
            document_id: None,
        };

        let outcome = doc_put(params)
            .await
            .unwrap_or_else(|e| panic!("doc_put panicked at offset={offset}: {e}"));
        assert!(
            !outcome.value.document_id.is_empty(),
            "doc_put must return non-empty document_id at offset={offset}"
        );
    }
}

/// 8.1.3 forget — clear_namespace must scrub the namespace so subsequent
/// recalls do not see the canary content. Failure-path / edge-case assertion
/// required by gitbooks/developing/testing-strategy.md.
#[tokio::test]
async fn clear_namespace_removes_canary_from_recall() {
    let _lock = env_lock();
    let tmp = test_root();
    let _home = EnvVarGuard::set_to_path("HOME", tmp.path());
    let workspace_path = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace_path).expect("create workspace dir");
    let _ws = EnvVarGuard::set_to_path("OPENHUMAN_WORKSPACE", &workspace_path);
    ensure_memory_seams(&workspace_path);

    // Seed the namespace.
    doc_put(put_params()).await.expect("seed doc_put");

    // Pre-clear sanity: canary visible.
    let pre = memory_recall_memories(recall_request())
        .await
        .expect("pre-clear recall");
    let pre_blob = serde_json::to_string(&pre.value).expect("serialise pre");
    assert!(
        pre_blob.contains(CONTENT) || pre_blob.contains(KEY),
        "canary must be visible before clear — got {pre_blob}"
    );

    // Clear the namespace.
    let clear_outcome = clear_namespace(ClearNamespaceParams {
        namespace: NS.to_string(),
    })
    .await
    .expect("clear_namespace rpc");
    assert!(
        clear_outcome.value.cleared,
        "clear_namespace must report cleared=true"
    );
    assert_eq!(clear_outcome.value.namespace, NS);

    // Post-clear: canary must no longer surface in recall.
    let post = memory_recall_memories(recall_request())
        .await
        .expect("post-clear recall");
    let post_blob = serde_json::to_string(&post.value).expect("serialise post");
    assert!(
        !post_blob.contains(CONTENT),
        "canary content must be absent after clear — got {post_blob}"
    );
    assert!(
        !post_blob.contains(KEY),
        "canary key must be absent after clear — got {post_blob}"
    );
}

// ── A1: `openhuman.memory_namespace_summaries` ───────────────────────
//
// Gate controller `memory_namespace_summaries` had zero references under
// `tests/` or `app/test/` before this test.
//
// The handler (`memory/ops/documents.rs`) resolves `active_memory_guard()` and
// calls `MemoryCore::namespaces()`, which crosses tinybus into the downloaded
// tinymemory cdylib. That boundary is the reason this lives at the RI layer: a
// unit or renderer test would assert the shape of a response the host itself
// assembled and would prove nothing about the engine that produced the counts.
//
// The counts are the point, not the shape. The handler's own doc comment names
// the field finding it exists to serve (#5932): "a Gmail source that stored
// 1,120 documents should be checkable as exactly that number". A regression
// that returns an empty namespace list, or counts that are silently zero, is
// worse than an error because the number stays plausible.

/// Seed `count` distinct documents into `namespace` under one shared key
/// prefix. Distinct keys, because `doc_put` upserts on key — reusing one would
/// store a single document and make a count assertion pass for the wrong
/// reason.
async fn seed_namespace(namespace: &str, count: usize) {
    for i in 0..count {
        let params = PutDocParams {
            namespace: namespace.to_string(),
            key: format!("summaries-seed-{i}"),
            title: format!("Namespace summary seed {i}"),
            content: format!("Namespace summary seed body {i}"),
            source_type: "doc".to_string(),
            priority: "medium".to_string(),
            tags: Vec::new(),
            metadata: serde_json::Value::Null,
            category: "core".to_string(),
            session_id: None,
            document_id: None,
        };
        doc_put(params).await.expect("seed doc_put for summaries");
    }
}

/// `memory_namespace_summaries` reports per-namespace document counts that
/// match what was actually stored, and a total consistent with its own rows.
///
/// Two namespaces with **different, non-zero** counts, so the assertion cannot
/// be satisfied by an all-zero response or by a handler that returns one
/// namespace's count for every row.
#[tokio::test]
async fn namespace_summaries_report_per_namespace_counts_and_total() {
    let _lock = env_lock();
    let tmp = test_root();
    let _home = EnvVarGuard::set_to_path("HOME", tmp.path());
    let workspace_path = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace_path).expect("create workspace dir");
    let _ws = EnvVarGuard::set_to_path("OPENHUMAN_WORKSPACE", &workspace_path);
    ensure_memory_seams(&workspace_path);

    const NS_A: &str = "memory-summaries-e2e-alpha";
    const NS_B: &str = "memory-summaries-e2e-beta";
    const COUNT_A: usize = 3;
    const COUNT_B: usize = 5;

    // Start from a known state: sibling tests in this target share the process
    // and the workspace, so these two namespaces are cleared first rather than
    // assumed empty.
    for ns in [NS_A, NS_B] {
        clear_namespace(ClearNamespaceParams {
            namespace: ns.to_string(),
        })
        .await
        .expect("pre-clear namespace");
    }

    seed_namespace(NS_A, COUNT_A).await;
    seed_namespace(NS_B, COUNT_B).await;

    let outcome = memory_namespace_summaries()
        .await
        .expect("memory_namespace_summaries rpc");
    let response = outcome.value;

    let find = |wanted: &str| {
        response
            .namespaces
            .iter()
            .find(|row| row.namespace == wanted)
            .unwrap_or_else(|| {
                panic!(
                    "namespace {wanted} missing from summaries — got {:?}",
                    response
                        .namespaces
                        .iter()
                        .map(|row| (&row.namespace, row.count))
                        .collect::<Vec<_>>()
                )
            })
    };

    let alpha = find(NS_A);
    let beta = find(NS_B);

    assert_eq!(
        alpha.count, COUNT_A,
        "namespace {NS_A} stored {COUNT_A} documents, summaries reported {}",
        alpha.count
    );
    assert_eq!(
        beta.count, COUNT_B,
        "namespace {NS_B} stored {COUNT_B} documents, summaries reported {}",
        beta.count
    );
    // The two counts differ, so this also rules out a handler that reports one
    // row's count for every namespace.
    assert_ne!(
        alpha.count, beta.count,
        "the fixture uses different counts per namespace; equal counts mean the \
         per-namespace number is not being read per namespace"
    );

    // `total_documents` is summed by the handler over the rows it returns, so it
    // must agree with those rows. This catches a total computed from a
    // different source, or one that silently saturates, without depending on
    // what sibling tests left in other namespaces.
    let summed: u64 = response.namespaces.iter().map(|row| row.count as u64).sum();
    assert_eq!(
        response.total_documents, summed,
        "total_documents ({}) must equal the sum of the returned rows ({summed})",
        response.total_documents
    );
    // And it must at least account for what this test stored.
    assert!(
        response.total_documents >= (COUNT_A + COUNT_B) as u64,
        "total_documents ({}) is below the {} documents this test stored",
        response.total_documents,
        COUNT_A + COUNT_B
    );
}

// ── A8: the ingest queue drains at a size that actually queues ───────────────
//
// Matrix 8.2.3 "Memory Scaling" is 🟡: ingestion moved into the tinymemory
// module, and the host kept no test that a batch large enough to queue is
// actually DRAINED rather than silently dropped at the bus boundary. 8.2.2
// covers ingest-reflected-by-status, but at a size that never queues, so the
// path where documents are accepted and then lost has no coverage at all.
//
// This is deliberately a bounded scale check and not a soak. A soak belongs in
// a lane that can afford minutes; what is missing here is cheap and specific:
// does the count still reconcile once the write is big enough to be batched?
// Losing documents silently is the failure mode that a small-N test cannot see,
// because a queue that drops its tail still looks correct at N = 3.
//
// If this proves slow in CI, lower `SCALE_DOCS` rather than deleting the test —
// the assertion is the reconciliation, not the number.

/// Ingesting a batch large enough to queue leaves an exact, reconciled count.
#[tokio::test]
async fn namespace_summaries_reconcile_after_a_batch_large_enough_to_queue() {
    let _lock = env_lock();
    let tmp = test_root();
    let _home = EnvVarGuard::set_to_path("HOME", tmp.path());
    let workspace_path = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace_path).expect("create workspace dir");
    let _ws = EnvVarGuard::set_to_path("OPENHUMAN_WORKSPACE", &workspace_path);
    ensure_memory_seams(&workspace_path);

    const NS_SCALE: &str = "memory-summaries-e2e-scale";
    const SCALE_DOCS: usize = 60;

    clear_namespace(ClearNamespaceParams {
        namespace: NS_SCALE.to_string(),
    })
    .await
    .expect("pre-clear scale namespace");

    // Fixture guard: prove the namespace really is empty before the batch, so
    // a count of SCALE_DOCS afterwards cannot be inherited from a sibling test
    // that used the same name.
    let before = memory_namespace_summaries()
        .await
        .expect("summaries before scale ingest");
    let before_count = before
        .value
        .namespaces
        .iter()
        .find(|row| row.namespace == NS_SCALE)
        .map(|row| row.count)
        .unwrap_or(0);
    assert_eq!(
        before_count, 0,
        "scale namespace must start empty; found {before_count} documents"
    );

    seed_namespace(NS_SCALE, SCALE_DOCS).await;

    let after = memory_namespace_summaries()
        .await
        .expect("summaries after scale ingest");
    let row = after
        .value
        .namespaces
        .iter()
        .find(|row| row.namespace == NS_SCALE)
        .unwrap_or_else(|| {
            panic!(
                "scale namespace vanished from summaries after ingesting {SCALE_DOCS} documents \
                 — got {:?}",
                after
                    .value
                    .namespaces
                    .iter()
                    .map(|r| (&r.namespace, r.count))
                    .collect::<Vec<_>>()
            )
        });

    assert_eq!(
        row.count, SCALE_DOCS,
        "ingested {SCALE_DOCS} documents but the namespace reports {}. A batch that queues must \
         still reconcile exactly; a short count here means writes were accepted and then dropped \
         between the host and the module, which is invisible at the sizes 8.2.2 exercises.",
        row.count
    );
}
