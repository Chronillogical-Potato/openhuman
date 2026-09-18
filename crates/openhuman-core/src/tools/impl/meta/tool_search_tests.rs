// The tokenizer's own tests live with it, in `util::bm25`.
use super::*;

fn spec(name: &str, description: &str) -> ToolSpec {
    ToolSpec {
        name: name.to_string(),
        description: description.to_string(),
        parameters: json!({"type": "object"}),
    }
}

fn index() -> ToolSearchIndex {
    ToolSearchIndex::build(&[
        spec("stock_quote", "Get the latest price for a stock ticker"),
        spec("cron_add", "Schedule a recurring job to run later"),
        spec(
            "memory_hybrid_search",
            "Search stored memories semantically",
        ),
        spec(
            "generate_presentation",
            "Build a pptx slide deck from an outline",
        ),
    ])
}

#[test]
fn a_plain_language_query_finds_the_right_tool() {
    let index = index();
    let hits = index.search("schedule something to run every morning", 3);
    assert_eq!(hits.first().map(|t| t.name.as_str()), Some("cron_add"));
}

#[test]
fn a_query_matching_the_name_rather_than_the_description_still_hits() {
    let index = index();
    let hits = index.search("stock", 3);
    assert_eq!(hits.first().map(|t| t.name.as_str()), Some("stock_quote"));
}

#[test]
fn nothing_relevant_returns_nothing_rather_than_padding_to_the_limit() {
    // Padding would spend exactly the tokens deferral saves, and would
    // invite a call to something unrelated to the ask.
    let index = index();
    assert!(index.search("xyzzy quantum flux", 5).is_empty());
}

#[test]
fn results_are_capped_at_the_requested_limit() {
    let index = index();
    assert!(index.search("search a stock job memory slide", 2).len() <= 2);
}

#[test]
fn an_empty_query_matches_nothing() {
    let index = index();
    assert!(index.search("   ", 5).is_empty());
}

#[test]
fn an_empty_index_is_searchable_without_panicking() {
    // `average_length` is 0 here; the length-normalisation term divides by
    // it, so this is the case that would panic or produce NaN if the
    // `.max(1.0)` guard were dropped.
    let empty = ToolSearchIndex::build(&[]);
    assert!(empty.is_empty());
    assert!(empty.search("anything", 5).is_empty());
}

#[test]
fn ranking_is_stable_across_identical_queries() {
    let index = index();
    let first: Vec<&str> = index
        .search("search", 4)
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    let second: Vec<&str> = index
        .search("search", 4)
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert_eq!(first, second);
}

// ── strip + bind: a deferred tool leaves the wire and stays findable ─────────

struct ExposureTestTool {
    name: &'static str,
    description: &'static str,
    exposure: tinytools::ToolExposure,
}

#[async_trait::async_trait]
impl Tool for ExposureTestTool {
    fn name(&self) -> &str {
        self.name
    }

    fn description(&self) -> &str {
        self.description
    }

    fn parameters_schema(&self) -> Value {
        json!({"type": "object"})
    }

    fn exposure(&self) -> tinytools::ToolExposure {
        self.exposure
    }

    async fn execute(&self, _args: Value) -> Result<ToolResult> {
        Ok(ToolResult::success("ok"))
    }
}

/// The two halves the session builder runs back to back. A Deferred tool must
/// come off the advertised set AND be findable through `tool_search`; a Hidden
/// one comes off and is not searchable; `tool_search` itself always stays.
/// Stripping without the index would make every deferred tool unreachable.
#[test]
fn a_deferred_tool_leaves_the_wire_and_is_found_by_tool_search() {
    use tinytools::ToolExposure;
    let tools: Vec<Box<dyn Tool>> = vec![
        Box::new(ToolSearchTool::new()),
        Box::new(ExposureTestTool {
            name: "stock_quote",
            description: "Get the latest price for a stock ticker",
            exposure: ToolExposure::Deferred,
        }),
        Box::new(ExposureTestTool {
            name: "memory_store",
            description: "Store a memory",
            exposure: ToolExposure::Hidden,
        }),
        Box::new(ExposureTestTool {
            name: "memory",
            description: "Store and search memories",
            exposure: ToolExposure::Direct,
        }),
    ];
    let mut visible: std::collections::HashSet<String> =
        tools.iter().map(|t| t.name().to_string()).collect();

    let deferred = strip_deferred_from_visible(&mut visible, &tools);
    assert!(bind_tool_search_index(&tools, deferred));

    let mut advertised: Vec<&str> = visible.iter().map(String::as_str).collect();
    advertised.sort_unstable();
    assert_eq!(advertised, vec!["memory", TOOL_SEARCH_NAME]);

    let handle = tools[0]
        .host_extension()
        .and_then(|any| any.downcast_ref::<ToolSearchHandle>())
        .expect("tool_search exposes its index handle");
    let index = handle.read().unwrap();
    let hits: Vec<&str> = index
        .search("stock price", 5)
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert_eq!(
        hits,
        vec!["stock_quote"],
        "the deferred tool must be searchable"
    );
    assert!(
        index.search("store memory", 5).is_empty(),
        "a Hidden tool is not searchable"
    );
}
