use super::*;

use std::sync::Arc;

use tinyagents_harness::limits::RunLimits;
use tinyagents_harness::runtime::{AgentHarness, RunPolicy};
use tinyagents_harness::testkit::{FakeTool, ScriptedModel};
use tinyagents_harness::tinyinference_llm::tool::ToolCall;

/// The real request this middleware was built for names three inputs and one
/// output. Extraction deliberately cannot tell them apart -- that is the
/// existence check's job -- so it must return all four.
const ATRX_REQUEST: &str = "Catalogue all coding variants present in the mutated ATRX \
    transcripts at /app/data/mutated-transcripts.txt relative to the wild-type NM_000489.6 \
    reference (encoded by /app/data/genomic-locus.fa and the CDS information at \
    /app/data/CDS-information.txt). Write the final results to \
    /app/output/mutation.report.json as a single JSON object.";

#[test]
fn extraction_returns_every_named_file_input_and_output_alike() {
    assert_eq!(
        candidate_paths(ATRX_REQUEST),
        vec![
            "/app/data/mutated-transcripts.txt",
            "/app/data/genomic-locus.fa",
            "/app/data/CDS-information.txt",
            "/app/output/mutation.report.json",
        ]
    );
}

/// The design claim: separating a deliverable from an input needs no grammar,
/// because an input is on disk and a skipped deliverable is not.
#[test]
fn existence_alone_separates_a_skipped_deliverable_from_the_inputs() {
    let dir = std::env::temp_dir().join(format!("oh-unmet-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let input = dir.join("given.txt");
    std::fs::write(&input, b"provided").expect("write input");
    let output = dir.join("report.json");
    let _ = std::fs::remove_file(&output);

    let candidates = vec![
        input.to_string_lossy().to_string(),
        output.to_string_lossy().to_string(),
    ];
    assert_eq!(
        UnmetDeliverableMiddleware::missing(&candidates),
        vec![output.to_string_lossy().to_string()],
        "the input must not be reported; only the path nothing created"
    );

    // Once written, even empty, it stops being reported: this middleware
    // checks existence, never contents.
    std::fs::write(&output, b"{}").expect("write output");
    assert!(UnmetDeliverableMiddleware::missing(&candidates).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn only_absolute_paths_carrying_a_file_extension_are_candidates() {
    for rejected in [
        "see config/settings.yml for the rest", // relative: a reference, not a deliverable
        "write it under /app/output/",          // a directory, no extension
        "results go in /report.json",           // single segment at the root
        "read /app/../etc/passwd",              // traversal is never statted
        "the ratio is 3/4.5 overall",           // arithmetic, not a path
    ] {
        assert!(
            candidate_paths(rejected).is_empty(),
            "must not treat {rejected:?} as a deliverable: {:?}",
            candidate_paths(rejected)
        );
    }
}

#[test]
fn a_path_is_recognised_through_the_punctuation_a_request_wraps_it_in() {
    for (text, expected) in [
        ("write to `/app/out/r.json`.", "/app/out/r.json"),
        ("write to \"/app/out/r.json\",", "/app/out/r.json"),
        ("write to (/app/out/r.json)", "/app/out/r.json"),
        ("write to /app/out/r.json.", "/app/out/r.json"),
        ("write to </app/out/r.json>", "/app/out/r.json"),
    ] {
        assert_eq!(candidate_paths(text), vec![expected], "for {text:?}");
    }
}

#[test]
fn a_repeated_path_is_reported_once_and_a_long_list_is_bounded() {
    let twice = "first /a/b/c.json then /a/b/c.json again";
    assert_eq!(candidate_paths(twice), vec!["/a/b/c.json"]);

    let many = (0..20)
        .map(|i| format!("/dir/file{i}.json"))
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(candidate_paths(&many).len(), MAX_CANDIDATES);
}

#[test]
fn the_notice_names_the_paths_and_asks_for_a_partial_file() {
    let one = notice(&["/app/output/r.json".to_string()]);
    assert!(one.contains("<harness_instruction>"));
    assert!(one.contains("a file that does not exist"));
    assert!(one.contains("`/app/output/r.json`"));
    assert!(one.contains("even where fields are incomplete or provisional"));

    let two = notice(&["/a/x.json".to_string(), "/b/y.csv".to_string()]);
    assert!(two.contains("files that do not exist"));
    assert!(two.contains("`/a/x.json`, `/b/y.csv`"));
}

fn tool_round(id: &str, name: &str) -> tinyagents_harness::tinyinference_llm::model::ModelResponse {
    let mut response =
        tinyagents_harness::tinyinference_llm::model::ModelResponse::assistant(String::new());
    response.message.content = Vec::new();
    response.message.tool_calls = vec![ToolCall::new(id, name, serde_json::json!({}))];
    response.finish_reason = Some("tool_calls".to_string());
    response
}

/// Drive a run whose request names a path that does not exist. The notice has
/// to reach the model, leave it free to call tools, and let its second answer
/// stand -- and it must not be given twice.
#[tokio::test]
async fn an_unwritten_deliverable_holds_the_answer_once_and_permits_a_fix() {
    let missing = std::env::temp_dir().join(format!("oh-unmet-run-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&missing);
    let request = format!("Do the work and write it to {}", missing.display());

    let mut harness: AgentHarness<()> = AgentHarness::new();
    harness.register_model(
        "mock",
        Arc::new(ScriptedModel::new(vec![
            tool_round("c0", "writer"),
            tinyagents_harness::tinyinference_llm::model::ModelResponse::assistant(
                "here is what I found".to_string(),
            ),
            tool_round("c1", "writer"),
            tinyagents_harness::tinyinference_llm::model::ModelResponse::assistant(
                "written and answered".to_string(),
            ),
            tinyagents_harness::tinyinference_llm::model::ModelResponse::assistant(
                "must not be reached".to_string(),
            ),
        ])),
    );
    harness.register_tool(Arc::new(FakeTool::returning("writer", "ok")));
    harness.with_policy(RunPolicy {
        limits: RunLimits::default()
            .with_max_model_calls(20)
            .with_max_tool_calls(20),
        ..RunPolicy::default()
    });
    harness.push_middleware(Arc::new(UnmetDeliverableMiddleware::new()));

    let run = harness
        .invoke_default(&(), vec![Message::user(request)])
        .await
        .expect("run succeeds");

    assert_eq!(run.text().as_deref(), Some("written and answered"));
    let notices = run
        .messages
        .iter()
        .filter(|m| matches!(m, Message::User(_)) && m.text().contains("does not exist"))
        .count();
    assert_eq!(
        notices, 1,
        "the notice must be given once, not every answer"
    );
}

/// A request that names nothing, or names only files that exist, must cost the
/// turn nothing: the first answer stands.
#[tokio::test]
async fn a_request_naming_no_missing_file_is_left_alone() {
    let mut harness: AgentHarness<()> = AgentHarness::new();
    harness.register_model(
        "mock",
        Arc::new(ScriptedModel::new(vec![
            tool_round("c0", "writer"),
            tinyagents_harness::tinyinference_llm::model::ModelResponse::assistant(
                "done".to_string(),
            ),
            tinyagents_harness::tinyinference_llm::model::ModelResponse::assistant(
                "must not be reached".to_string(),
            ),
        ])),
    );
    harness.register_tool(Arc::new(FakeTool::returning("writer", "ok")));
    harness.with_policy(RunPolicy {
        limits: RunLimits::default().with_max_model_calls(20),
        ..RunPolicy::default()
    });
    harness.push_middleware(Arc::new(UnmetDeliverableMiddleware::new()));

    let run = harness
        .invoke_default(&(), vec![Message::user("summarise the situation for me")])
        .await
        .expect("run succeeds");
    assert_eq!(run.text().as_deref(), Some("done"));
}
