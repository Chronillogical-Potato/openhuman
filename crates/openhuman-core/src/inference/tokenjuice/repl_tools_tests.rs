use super::*;
use serde_json::json;
use tinyjuice::cache::store::MemoryCcrStore;
use tinyjuice::types::{CompressOptions, CompressorKind};

/// Reads originals from an in-process store instead of the module.
struct MemorySource(Arc<MemoryCcrStore>);

#[async_trait]
impl OriginalSource for MemorySource {
    async fn original(&self, handle: &str) -> Result<Option<String>, String> {
        Ok(self.0.get(handle))
    }
}

struct FailingSource;

#[async_trait]
impl OriginalSource for FailingSource {
    async fn original(&self, _handle: &str) -> Result<Option<String>, String> {
        Err("module unavailable".to_string())
    }
}

fn tool(tools: &[Box<dyn Tool>], name: &str) -> usize {
    tools
        .iter()
        .position(|t| t.name() == name)
        .unwrap_or_else(|| panic!("no tool named {name}"))
}

fn log_body() -> String {
    let rows: String = (0..900)
        .map(|i| format!("row {i}: value {}\n", i * 7))
        .collect();
    let tail: String = (900..1000)
        .map(|i| format!("row {i}: value {}\n", i * 7))
        .collect();
    format!("# Report\n{rows}ERROR: needle in the middle\n{tail}")
}

/// Compress `content` in handle mode against `store`, returning the model-facing
/// text and the handle.
async fn store_behind_handle(store: &MemoryCcrStore, content: &str) -> (String, String) {
    let options = CompressOptions {
        repl_handle: true,
        ..CompressOptions::default()
    };
    let out = tinyjuice::compress_content_with_store(content, None, &options, store).await;
    assert_eq!(
        out.compressor,
        CompressorKind::Repl,
        "handle mode must apply"
    );
    let handle = out.ccr_token.clone().expect("a handle");
    (out.text, handle)
}

fn result_text(result: &ToolResult) -> String {
    result.output()
}

#[test]
fn declares_the_three_repl_tools_read_only_capped_and_small() {
    let tools = repl_tools();
    let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
    assert_eq!(
        names, REPL_TOOL_NAMES,
        "same tools, same order as TinyJuice"
    );

    let mut schema_bytes = 0;
    for t in &tools {
        assert!(is_repl_tool(t.name()));
        assert_eq!(t.permission_level(), PermissionLevel::ReadOnly);
        assert!(t.is_concurrency_safe(&json!({})));
        assert!(!t.external_effect());
        assert!(
            t.max_result_size_chars().is_some_and(|cap| cap > 0),
            "{} must declare a result cap",
            t.name()
        );
        let schema = t.parameters_schema();
        assert!(
            schema["required"]
                .as_array()
                .is_some_and(|r| r.contains(&json!("handle"))),
            "{} must require a handle",
            t.name()
        );
        schema_bytes += t.name().len() + t.description().len() + schema.to_string().len();
    }
    // The point of three tools is a schema that stays small on every turn.
    assert!(schema_bytes < 3_000, "schemas are {schema_bytes} bytes");
}

#[tokio::test]
async fn handle_round_trip_find_summarize_and_extract() {
    let store = Arc::new(MemoryCcrStore::default());
    let content = log_body();
    let (preview, handle) = store_behind_handle(&store, &content).await;

    // The model sees a small preview naming the handle and the tools.
    assert!(preview.len() < content.len() / 4);
    assert!(preview.contains(&handle));
    // It names the slice read and the outline, then the whole-original
    // retrieve last; `juice_extract` (HTML/Markdown only) is left to its
    // description.
    for name in ["juice_find", "juice_summarize"] {
        assert!(preview.contains(name), "footer must name {name}");
    }
    let find = preview.find("juice_find").unwrap();
    let retrieve = preview
        .find(crate::inference::tokenjuice::RETRIEVE_TOOL_NAME)
        .expect("footer names the whole-original retrieve");
    assert!(
        find < retrieve,
        "slice read before whole original: {preview}"
    );
    assert!(
        !preview.contains("needle in the middle"),
        "the needle must be behind the handle, not in the preview"
    );

    let tools = repl_tools_with(Arc::new(MemorySource(store)), ReplLimits::default());

    let found = tools[tool(&tools, "juice_find")]
        .execute(json!({ "handle": handle, "query": "needle" }))
        .await
        .unwrap();
    assert!(!found.is_error, "{}", result_text(&found));
    assert!(result_text(&found).contains("needle in the middle"));

    let by_grep = tools[tool(&tools, "juice_find")]
        .execute(json!({ "handle": handle, "query": "^row 899:", "mode": "grep" }))
        .await
        .unwrap();
    assert!(result_text(&by_grep).contains("row 899"));

    let summary = tools[tool(&tools, "juice_summarize")]
        .execute(json!({ "handle": handle, "hint": "needle" }))
        .await
        .unwrap();
    assert!(!summary.is_error, "{}", result_text(&summary));

    let md = "# Title\ntext\n## Section\nmore\n".repeat(200);
    let md_store = Arc::new(MemoryCcrStore::default());
    let (_, md_handle) = store_behind_handle(&md_store, &md).await;
    let md_tools = repl_tools_with(Arc::new(MemorySource(md_store)), ReplLimits::default());
    let headings = md_tools[tool(&md_tools, "juice_extract")]
        .execute(json!({ "handle": md_handle, "what": "headings" }))
        .await
        .unwrap();
    assert!(!headings.is_error, "{}", result_text(&headings));
    assert!(result_text(&headings).contains("Section"));
}

#[tokio::test]
async fn answers_stay_within_the_declared_cap() {
    let store = Arc::new(MemoryCcrStore::default());
    let content = log_body();
    let (_, handle) = store_behind_handle(&store, &content).await;
    let tools = repl_tools_with(Arc::new(MemorySource(store)), ReplLimits::default());
    let find = &tools[tool(&tools, "juice_find")];
    // Every line matches.
    let all = find
        .execute(json!({ "handle": handle, "query": "row" }))
        .await
        .unwrap();
    let cap = find.max_result_size_chars().unwrap();
    assert!(
        result_text(&all).chars().count() <= cap,
        "{} chars exceeds the {cap} cap",
        result_text(&all).chars().count()
    );
    assert!(result_text(&all).len() < content.len() / 4);
}

#[tokio::test]
async fn the_marker_form_of_a_handle_is_accepted() {
    let store = Arc::new(MemoryCcrStore::default());
    let (_, handle) = store_behind_handle(&store, &log_body()).await;
    let tools = repl_tools_with(Arc::new(MemorySource(store)), ReplLimits::default());
    let res = tools[0]
        .execute(json!({ "handle": format!("⟦tj:{handle}⟧"), "query": "needle" }))
        .await
        .unwrap();
    assert!(!res.is_error, "{}", result_text(&res));
    assert!(result_text(&res).contains("needle"));
}

#[tokio::test]
async fn bad_or_unknown_handles_are_errors_that_do_not_invite_a_re_run() {
    let store = Arc::new(MemoryCcrStore::default());
    let tools = repl_tools_with(Arc::new(MemorySource(store)), ReplLimits::default());
    let find = &tools[tool(&tools, "juice_find")];

    let missing = find.execute(json!({ "query": "x" })).await.unwrap();
    assert!(missing.is_error);

    for bad in ["", "../etc/passwd", "a b", &"a".repeat(200)] {
        let res = find
            .execute(json!({ "handle": bad, "query": "x" }))
            .await
            .unwrap();
        assert!(res.is_error, "handle {bad:?} must be rejected");
    }

    let unknown = find
        .execute(json!({ "handle": "deadbeefdeadbeef", "query": "x" }))
        .await
        .unwrap();
    assert!(unknown.is_error);
    let msg = result_text(&unknown).to_lowercase();
    assert!(msg.contains("do not re-run"), "{msg}");
}

#[tokio::test]
async fn a_source_failure_is_reported_not_raised() {
    let tools = repl_tools_with(Arc::new(FailingSource), ReplLimits::default());
    let res = tools[0]
        .execute(json!({ "handle": "abc123", "query": "x" }))
        .await
        .unwrap();
    assert!(res.is_error);
    assert!(result_text(&res).contains("module unavailable"));
}

#[test]
fn handle_normalization_only_accepts_alphanumeric_tokens() {
    assert_eq!(normalize_handle(" abc123 "), Some("abc123"));
    assert_eq!(normalize_handle("⟦tj:abc123⟧"), Some("abc123"));
    assert_eq!(normalize_handle(""), None);
    assert_eq!(normalize_handle("../x"), None);
    assert_eq!(normalize_handle(&"f".repeat(MAX_HANDLE_LEN + 1)), None);
}

#[test]
fn repl_tools_share_the_harness_tool_trait() {
    // `tinyjuice::repl::tools` and this crate must name one `tinytools::Tool`;
    // this only compiles when `repl_tools()` yields the trait objects the tool
    // registry (`Vec<Box<dyn tinytools::Tool>>`) takes.
    let registry: Vec<Box<dyn tinytools::Tool>> = repl_tools();
    assert_eq!(registry.len(), REPL_TOOL_NAMES.len());
    let raw: Vec<Box<dyn tinytools::Tool>> = tinyjuice::repl::tools::repl_tools(
        Arc::new(MemoryCcrStore::default()),
        ReplLimits::default(),
    );
    assert_eq!(raw.len(), registry.len());
}
